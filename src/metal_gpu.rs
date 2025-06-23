#[cfg(target_os = "macos")]
use crate::{Config, RewardVariant, SaltVariant, CreateXVariant};
use alloy_primitives::{hex, FixedBytes};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use console::Term;
use fs4::FileExt;
use itertools::chain;
use metal::{Device, MTLResourceOptions};
use rand::{thread_rng, Rng};
use separator::Separatable;
use std::{
    fs::{File, OpenOptions},
    io::prelude::*,
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_size::{terminal_size, Height};

const WORK_SIZE: u32 = 0x4000000;
const WORK_FACTOR: u128 = (WORK_SIZE as u128) / 1_000_000;

#[track_caller]
fn output_file(config: &Config) -> File {
    OpenOptions::new()
        .append(true)
        .create(true)
        .read(true)
        .open(config.output)
        .unwrap_or_else(|_| panic!("Could not create or open {} file.", config.output))
}

pub fn gpu_metal(config: Config) -> Result<(), String> {
    println!(
        "Setting up Metal compute pipeline using device {}...",
        config.gpu_device
    );

    // Get the default Metal device
    let device = Device::system_default()
        .ok_or_else(|| "No Metal device found. Metal is only available on macOS.".to_string())?;

    println!("Using Metal device: {}", device.name());

    // Create command queue
    let command_queue = device.new_command_queue();

    // Generate kernel source with config
    let kernel_src = crate::metal_kernel::mk_metal_kernel_src(&config);

    // Compile the Metal shader
    let library = device
        .new_library_with_source(&kernel_src, &metal::CompileOptions::new())
        .map_err(|e| format!("Failed to compile Metal shader: {}", e))?;

    let kernel_function = library
        .get_function("hashMessage", None)
        .map_err(|_| "Failed to get kernel function")?;

    // Create compute pipeline state
    let pipeline_state = device
        .new_compute_pipeline_state_with_function(&kernel_function)
        .map_err(|e| format!("Failed to create pipeline state: {}", e))?;

    // Create buffers for input/output
    let message_buffer = device.new_buffer(4, MTLResourceOptions::StorageModeShared);
    let nonce_buffer = device.new_buffer(4, MTLResourceOptions::StorageModeShared);
    let solutions_buffer = device.new_buffer(32, MTLResourceOptions::StorageModeShared); // 4 * u64
    
    // Create empty config buffer (not used in templated approach)
    let config_buffer = device.new_buffer(1, MTLResourceOptions::StorageModeShared);

    // Open output file
    let file = output_file(&config);

    // Track statistics
    let mut found: u64 = 0;
    let mut found_list: Vec<String> = vec![];
    let term = Term::stdout();
    let mut rng = thread_rng();
    let start_time: f64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let mut rate: f64 = 0.0;
    let mut cumulative_nonce: u64 = 0;
    let mut previous_time: f64 = 0.0;
    let mut work_duration_millis: u64 = 0;

    // Main mining loop
    loop {
        // Generate random salt
        let salt = FixedBytes::<4>::random();
        
        // Update message buffer
        unsafe {
            let msg_ptr = message_buffer.contents() as *mut u8;
            std::ptr::copy_nonoverlapping(salt.as_ptr(), msg_ptr, 4);
        }

        // Initialize nonce
        let mut nonce: [u32; 1] = rng.gen();
        let mut view_buf = [0; 8];

        // Update nonce buffer
        unsafe {
            let nonce_ptr = nonce_buffer.contents() as *mut u32;
            *nonce_ptr = nonce[0];
        }

        // Clear solutions buffer
        unsafe {
            let solutions_ptr = solutions_buffer.contents() as *mut u64;
            std::ptr::write_bytes(solutions_ptr, 0, 4);
        }

        // Inner loop for incrementing nonce
        loop {
            // Create command buffer
            let command_buffer = command_queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();
            
            encoder.set_compute_pipeline_state(&pipeline_state);
            encoder.set_buffer(0, Some(&message_buffer), 0);
            encoder.set_buffer(1, Some(&nonce_buffer), 0);
            encoder.set_buffer(2, Some(&solutions_buffer), 0);
            encoder.set_buffer(3, Some(&config_buffer), 0);

            // Calculate thread configuration
            // Use optimal thread group size for compute workloads
            let threads_per_threadgroup = 256u64.min(pipeline_state.max_total_threads_per_threadgroup());
            let total_threads = metal::MTLSize::new(WORK_SIZE as u64, 1, 1);
            let threadgroup_size = metal::MTLSize::new(threads_per_threadgroup, 1, 1);
            

            encoder.dispatch_threads(total_threads, threadgroup_size);
            encoder.end_encoding();
            
            command_buffer.commit();
            command_buffer.wait_until_completed();

            // Update timing and display
            let mut now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let current_time = now.as_secs() as f64;
            let print_output = current_time - previous_time > 0.99;
            previous_time = current_time;

            if print_output {
                term.clear_screen().ok();
                let total_runtime = current_time - start_time;
                let total_runtime_hrs = total_runtime as u64 / 3600;
                let total_runtime_mins = (total_runtime as u64 - total_runtime_hrs * 3600) / 60;
                let total_runtime_secs = total_runtime
                    - (total_runtime_hrs * 3600) as f64
                    - (total_runtime_mins * 60) as f64;

                let work_rate: u128 = WORK_FACTOR * cumulative_nonce as u128;
                if total_runtime > 0.0 {
                    rate = 1.0 / total_runtime;
                }

                LittleEndian::write_u64(&mut view_buf, (nonce[0] as u64) << 32);
                let height = terminal_size().map(|(_w, Height(h))| h).unwrap_or(10);

                term.write_line(&format!(
                    "total runtime: {}:{:02}:{:02} ({} cycles)\t\t\t\
                     work size per cycle: {}",
                    total_runtime_hrs,
                    total_runtime_mins,
                    total_runtime_secs,
                    cumulative_nonce,
                    WORK_SIZE.separated_string(),
                )).ok();

                term.write_line(&format!(
                    "rate: {:.2} million attempts per second\t\t\t\
                     total found this run: {}",
                    work_rate as f64 * rate,
                    found
                )).ok();

                let threshold_string = match config.reward {
                    RewardVariant::LeadingZeros { zeros_threshold } => {
                        format!("with {} leading zero byte(s)", zeros_threshold)
                    }
                    RewardVariant::TotalZeros { zeros_threshold } => {
                        format!("with {} total zero byte(s)", zeros_threshold)
                    }
                    RewardVariant::LeadingAndTotalZeros {
                        leading_zeros_threshold,
                        total_zeros_threshold,
                    } => format!(
                        "with {} leading and {} total zero byte(s)",
                        leading_zeros_threshold, total_zeros_threshold
                    ),
                    RewardVariant::LeadingOrTotalZeros {
                        leading_zeros_threshold,
                        total_zeros_threshold,
                    } => format!(
                        "with {} leading or {} total zero byte(s)",
                        leading_zeros_threshold, total_zeros_threshold
                    ),
                    RewardVariant::Matching { ref pattern } => {
                        format!("matching pattern 0x{}", pattern)
                    }
                };

                let variant = match config.create_variant {
                    CreateXVariant::Create2 { .. } => "Create2",
                    CreateXVariant::Create3 => "Create3",
                };

                term.write_line(&format!(
                    "current search space: {}xxxxxxxx{:06x}\t\t\
                     threshold: mining for {} address {}",
                    hex::encode(salt),
                    BigEndian::read_u64(&view_buf) >> 8,
                    variant,
                    threshold_string
                )).ok();

                let rows = if height < 5 { 1 } else { height as usize - 4 };
                let last_rows: Vec<String> = found_list.iter().cloned().rev().take(rows).collect();
                let ordered: Vec<String> = last_rows.iter().cloned().rev().collect();
                let recently_found = &ordered.join("\n");
                term.write_line(recently_found).ok();
            }

            cumulative_nonce += 1;
            let work_start_time_millis = now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1000000;

            if work_duration_millis != 0 {
                std::thread::sleep(std::time::Duration::from_millis(
                    work_duration_millis * 980 / 1000,
                ));
            }

            // Check solutions
            let mut solutions = vec![0u64; 4];
            unsafe {
                let solutions_ptr = solutions_buffer.contents() as *const u64;
                std::ptr::copy_nonoverlapping(solutions_ptr, solutions.as_mut_ptr(), 4);
            }

            now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            work_duration_millis = (now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1000000)
                - work_start_time_millis;

            if solutions[0] != 0 {
                break;
            }

            nonce[0] += 1;
            unsafe {
                let nonce_ptr = nonce_buffer.contents() as *mut u32;
                *nonce_ptr = nonce[0];
            }
        }

        // Process solution
        let mut solutions = vec![0u64; 4];
        unsafe {
            let solutions_ptr = solutions_buffer.contents() as *const u64;
            std::ptr::copy_nonoverlapping(solutions_ptr, solutions.as_mut_ptr(), 4);
        }

        let solution = solutions[0];
        let solution = solution.to_le_bytes();

        let mined_salt = chain!(salt, solution[..7].iter().copied());

        let salt: Vec<u8> = match config.salt_variant {
            SaltVariant::CrosschainSender {
                chain_id: _,
                calling_address,
            } => chain!(calling_address, [1u8], mined_salt).collect(),
            SaltVariant::Crosschain { chain_id: _ } => {
                chain!([0u8; 20], [1u8], mined_salt).collect()
            }
            SaltVariant::Sender { calling_address } => {
                chain!(calling_address, [0u8], mined_salt).collect()
            }
            SaltVariant::Random => chain!(mined_salt, [0u8; 21]).collect(),
        };

        let address = solutions[1]
            .to_be_bytes()
            .into_iter()
            .chain(solutions[2].to_be_bytes())
            .chain(solutions[3].to_be_bytes()[..4].to_vec())
            .collect::<Vec<u8>>();

        let mut total = 0;
        let mut leading = 0;
        for (i, &b) in address.iter().enumerate() {
            if b != 0 { continue; };

            if leading == i {
                leading = i + 1;
            }

            total += 1;
        }

        let output = format!("0x{} => 0x{}", hex::encode(salt), hex::encode(address));

        let show = format!("{output} ({leading} / {total})");
        match config.reward {
            RewardVariant::Matching { .. } => {
                found_list.push(output.to_string());
            }
            _ => {
                found_list.push(show);
            }
        }

        file.lock_exclusive().expect("Couldn't lock file.");
        writeln!(&file, "{output}").expect("Couldn't write to output file.");
        file.unlock().expect("Couldn't unlock file.");
        found += 1;
    }
}