use alloy_primitives::{hex, FixedBytes};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use console::Term;
use fs4::FileExt;
use ocl::{Buffer, Context, Device, MemFlags, Platform, ProQue, Program, Queue};
use rand::{thread_rng, Rng};
use separator::Separatable;
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::prelude::*,
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_size::{terminal_size, Height};

pub mod cli;

const PROXY_CHILD_CODEHASH: [u8; 32] = [
    33, 195, 93, 190, 27, 52, 74, 36, 136, 207, 51, 33, 214, 206, 84, 47, 142, 159, 48, 85, 68,
    255, 9, 228, 153, 58, 98, 49, 154, 73, 124, 31,
];

// workset size (tweak this!)
const WORK_SIZE: u32 = 0x4000000; // max. 0x15400000 to abs. max 0xffffffff

const WORK_FACTOR: u128 = (WORK_SIZE as u128) / 1_000_000;

static KERNEL_SRC: &str = include_str!("./kernels/keccak256.cl");

enum CreateXVariant {
    Create2 { init_code_hash: [u8; 32] },
    Create3,
}

pub enum RewardVariant {
    LeadingZeros { leading_zeros_threshold: u8 },
    Matching { pattern: String },
}

pub struct Config {
    gpu_device: u8,
    factory_address: [u8; 20],
    calling_address: [u8; 20],
    chain_id: Option<[u8; 32]>,
    variant: CreateXVariant,
    reward: RewardVariant,
    output: String,
}

impl Config {
    pub fn new(
        gpu_device: &str,
        factory_address: &String,
        calling_address: &String,
        chain_id: Option<&String>,
        init_code_hash: Option<&String>,
        reward: RewardVariant,
        output: &String,
    ) -> Result<Self, &'static str> {
        let chain_id = chain_id.map(|chain_id| {
            chain_id
                // Chain ids are technically 32 bytes, but 8 bytes should be enough
                .parse::<u64>()
                .expect("could not parse chain id argument as u64")
        });

        // convert main arguments from hex string to vector of bytes
        let factory_address_vec =
            hex::decode(factory_address).expect("could not decode factory address argument");
        let calling_address_vec =
            hex::decode(calling_address).expect("could not decode calling address argument");
        let init_code_hash_vec = init_code_hash.map(|init_code_hash| {
            hex::decode(init_code_hash).expect("could not decode init code hash argument")
        });

        // convert from vector to fixed array
        let factory_address = TryInto::<[u8; 20]>::try_into(factory_address_vec)
            .expect("invalid length for factory address argument");
        let calling_address = TryInto::<[u8; 20]>::try_into(calling_address_vec)
            .expect("invalid length for calling address argument");
        let init_code_hash = init_code_hash_vec.map(|init_code_hash_vec| {
            TryInto::<[u8; 32]>::try_into(init_code_hash_vec)
                .expect("invalid length for init code hash argument")
        });
        let chain_id = chain_id.map(|chain_id| {
            let mut arr = [0u8; 32];
            arr[24..].copy_from_slice(&chain_id.to_be_bytes());
            arr
        });

        let create_variant = match init_code_hash {
            Some(init_code_hash) => CreateXVariant::Create2 { init_code_hash },
            None => CreateXVariant::Create3 {},
        };

        // convert gpu argument to u8
        let Ok(gpu_device) = gpu_device.parse::<u8>() else {
            return Err("invalid gpu device value");
        };

        match &reward {
            RewardVariant::LeadingZeros {
                leading_zeros_threshold,
            } => {
                if leading_zeros_threshold == &0 {
                    return Err("leading zeros threshold must be greater than 0");
                }
                if leading_zeros_threshold > &20 {
                    return Err("leading zeros threshold must be less than 20");
                }
            }
            RewardVariant::Matching { pattern } => {
                if pattern.len() != 40 {
                    return Err("matching pattern must be 40 characters long");
                }
                if !pattern.chars().all(|c| c == 'X' || c.is_ascii_hexdigit()) {
                    return Err("matching pattern must only contain 'X' or hex characters");
                }
            }
        }

        Ok(Self {
            gpu_device,
            factory_address,
            calling_address,
            chain_id,
            variant: create_variant,
            reward,
            output: output.to_string(),
        })
    }
}

/// Adapted from https://github.com/0age/create2crunch
///
pub fn gpu(config: Config) -> ocl::Result<()> {
    println!(
        "Setting up experimental OpenCL miner using device {}...",
        config.gpu_device
    );

    let mut byte_array: [u8; 32] = [0; 32]; // Initialize with zeros

    // Decode the hexadecimal string and fill the byte array
    hex::decode_to_slice(
        "14a4ef128e0152790917bf1b0b28fcbdd871d03a79807e8be3e6a2263fe2039e",
        &mut byte_array,
    )
    .expect("Failed to decode hex");

    // (create if necessary) and open a file where found salts will be written
    let file = output_file(&config);

    // track how many addresses have been found and information about them
    let mut found: u64 = 0;
    let mut found_list: Vec<String> = vec![];

    // set up a controller for terminal output
    let term = Term::stdout();

    // set up a platform to use
    let platform = Platform::new(ocl::core::default_platform()?);

    // set up the device to use
    let device = Device::by_idx_wrap(platform, config.gpu_device as usize)?;

    // set up the context to use
    let context = Context::builder()
        .platform(platform)
        .devices(device)
        .build()?;

    // set up the program to use
    let program = Program::builder()
        .devices(device)
        .src(mk_kernel_src(&config))
        .build(&context)?;

    // set up the queue to use
    let queue = Queue::new(&context, device, None)?;

    // set up the "proqueue" (or amalgamation of various elements) to use
    let ocl_pq = ProQue::new(context, queue, program, Some(WORK_SIZE));

    // create a random number generator
    let mut rng = thread_rng();

    // determine the start time
    let start_time: f64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    // set up variables for tracking performance
    let mut rate: f64 = 0.0;
    let mut cumulative_nonce: u64 = 0;

    // the previous timestamp of printing to the terminal
    let mut previous_time: f64 = 0.0;

    // the last work duration in milliseconds
    let mut work_duration_millis: u64 = 0;

    // begin searching for addresses
    loop {
        // construct the 4-byte message to hash, leaving last 8 of salt empty
        let salt = FixedBytes::<4>::random();

        // build a corresponding buffer for passing the message to the kernel
        let message_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().read_only())
            .len(4)
            .copy_host_slice(&salt[..])
            .build()?;

        // reset nonce & create a buffer to view it in little-endian
        // for more uniformly distributed nonces, we shall initialize it to a random value
        let mut nonce: [u32; 1] = rng.gen();
        let mut view_buf = [0; 8];

        // build a corresponding buffer for passing the nonce to the kernel
        let mut nonce_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().read_only())
            .len(1)
            .copy_host_slice(&nonce)
            .build()?;

        // establish a buffer for nonces that result in desired addresses
        let mut solutions: Vec<u64> = vec![0; 4];
        let solutions_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().write_only())
            .len(4)
            .copy_host_slice(&solutions)
            .build()?;

        // repeatedly enqueue kernel to search for new addresses
        loop {
            // build the kernel and define the type of each buffer
            let kern = ocl_pq
                .kernel_builder("hashMessage")
                .arg_named("message", None::<&Buffer<u8>>)
                .arg_named("nonce", None::<&Buffer<u32>>)
                .arg_named("solutions", None::<&Buffer<u64>>)
                .build()?;

            // set each buffer
            kern.set_arg("message", Some(&message_buffer))?;
            kern.set_arg("nonce", Some(&nonce_buffer))?;
            kern.set_arg("solutions", &solutions_buffer)?;

            // enqueue the kernel
            unsafe { kern.enq()? };

            // calculate the current time
            let mut now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let current_time = now.as_secs() as f64;

            // we don't want to print too fast
            let print_output = current_time - previous_time > 0.99;
            previous_time = current_time;

            // clear the terminal screen
            if print_output {
                term.clear_screen()?;

                // get the total runtime and parse into hours : minutes : seconds
                let total_runtime = current_time - start_time;
                let total_runtime_hrs = total_runtime as u64 / 3600;
                let total_runtime_mins = (total_runtime as u64 - total_runtime_hrs * 3600) / 60;
                let total_runtime_secs = total_runtime
                    - (total_runtime_hrs * 3600) as f64
                    - (total_runtime_mins * 60) as f64;

                // determine the number of attempts being made per second
                let work_rate: u128 = WORK_FACTOR * cumulative_nonce as u128;
                if total_runtime > 0.0 {
                    rate = 1.0 / total_runtime;
                }

                // fill the buffer for viewing the properly-formatted nonce
                LittleEndian::write_u64(&mut view_buf, (nonce[0] as u64) << 32);

                // calculate the terminal height, defaulting to a height of ten rows
                let height = terminal_size().map(|(_w, Height(h))| h).unwrap_or(10);

                // display information about the total runtime and work size
                term.write_line(&format!(
                    "total runtime: {}:{:02}:{:02} ({} cycles)\t\t\t\
                     work size per cycle: {}",
                    total_runtime_hrs,
                    total_runtime_mins,
                    total_runtime_secs,
                    cumulative_nonce,
                    WORK_SIZE.separated_string(),
                ))?;

                // display information about the attempt rate and found solutions
                term.write_line(&format!(
                    "rate: {:.2} million attempts per second\t\t\t\
                     total found this run: {}",
                    work_rate as f64 * rate,
                    found
                ))?;

                let threshold_string = match config.reward {
                    RewardVariant::LeadingZeros {
                        leading_zeros_threshold,
                    } => format!("with {} leading zeros", leading_zeros_threshold),
                    RewardVariant::Matching { ref pattern } => {
                        format!("matching pattern {}", pattern)
                    }
                };

                let variant = match config.variant {
                    CreateXVariant::Create2 { init_code_hash: _ } => "Create2",
                    CreateXVariant::Create3 {} => "Create3",
                };

                // display information about the current search criteria
                term.write_line(&format!(
                    "current search space: {}xxxxxxxx{:08x}\t\t\
                     threshold: mining for {} address {}",
                    hex::encode(salt),
                    BigEndian::read_u64(&view_buf),
                    variant,
                    threshold_string
                ))?;

                // display recently found solutions based on terminal height
                let rows = if height < 5 { 1 } else { height as usize - 4 };
                let last_rows: Vec<String> = found_list.iter().cloned().rev().take(rows).collect();
                let ordered: Vec<String> = last_rows.iter().cloned().rev().collect();
                let recently_found = &ordered.join("\n");
                term.write_line(recently_found)?;
            }

            // increment the cumulative nonce (does not reset after a match)
            cumulative_nonce += 1;

            // record the start time of the work
            let work_start_time_millis = now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1000000;

            // sleep for 98% of the previous work duration to conserve CPU
            if work_duration_millis != 0 {
                std::thread::sleep(std::time::Duration::from_millis(
                    work_duration_millis * 980 / 1000,
                ));
            }

            // read the solutions from the device
            solutions_buffer.read(&mut solutions).enq()?;

            // record the end time of the work and compute how long the work took
            now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            work_duration_millis = (now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1000000)
                - work_start_time_millis;

            // if at least one solution is found, end the loop
            if solutions[0] != 0 {
                break;
            }

            // if no solution has yet been found, increment the nonce
            nonce[0] += 1;

            // update the nonce buffer with the incremented nonce value
            nonce_buffer = Buffer::builder()
                .queue(ocl_pq.queue().clone())
                .flags(MemFlags::new().read_write())
                .len(1)
                .copy_host_slice(&nonce)
                .build()?;
        }

        let solution = solutions[0];
        let solution = solution.to_le_bytes();

        let salt = if config.chain_id.is_some() && config.calling_address != [0u8; 20] {
            config
                .calling_address
                .into_iter()
                .chain([1u8])
                .chain(salt)
                .chain(solution[..7].iter().copied())
                .collect::<Vec<u8>>()
        } else if config.chain_id.is_none() && config.calling_address != [0u8; 20] {
            config
                .calling_address
                .into_iter()
                .chain([0u8])
                .chain(salt)
                .chain(solution[..7].iter().copied())
                .collect::<Vec<u8>>()
        } else if config.chain_id.is_some() && config.calling_address == [0u8; 20] {
            config
                .calling_address
                .into_iter()
                .chain([1u8])
                .chain(salt)
                .chain(solution[..7].iter().copied())
                .collect::<Vec<u8>>()
        } else {
            salt.into_iter()
                .chain(solution[..7].iter().copied())
                .chain([0u8; 21])
                .collect::<Vec<u8>>()
        };

        // get the address that results from the hash
        // let address = compute_create3_address(&config.factory_address, salt.as_slice());
        let address = solutions[1]
            .to_be_bytes()
            .into_iter()
            .chain(solutions[2].to_be_bytes())
            .chain(solutions[3].to_be_bytes()[..4].to_vec())
            .collect::<Vec<u8>>();

        // count total and leading zero bytes
        let mut _total = 0;
        let mut leading = 0;
        for (i, &b) in address.iter().enumerate() {
            if b == 0 {
                _total += 1;
            } else if leading == 0 {
                // set leading on finding non-zero byte
                leading = i;
            }
        }

        let output = format!("0x{} => {}", hex::encode(salt), hex::encode(address),);

        let show = format!("{output} ({leading})");
        found_list.push(show.to_string());

        file.lock_exclusive().expect("Couldn't lock file.");

        writeln!(&file, "{output}").expect("Couldn't write to `output.txt` file.");

        file.unlock().expect("Couldn't unlock file.");
        found += 1;
    }
}

#[track_caller]
fn output_file(config: &Config) -> File {
    OpenOptions::new()
        .append(true)
        .create(true)
        .read(true)
        .open(config.output.clone())
        .unwrap_or_else(|_| panic!("Could not create or open {} file.", config.output))
}

/// Creates the OpenCL kernel source code by populating the template with the
/// values from the Config object.
fn mk_kernel_src(config: &Config) -> String {
    let mut src = String::with_capacity(2048 + KERNEL_SRC.len());

    if config.chain_id.is_some() && config.calling_address != [0u8; 20] {
        writeln!(src, "#define GENERATE_SEED() SENDER_XCHAIN()").unwrap();
    } else if config.chain_id.is_none() && config.calling_address != [0u8; 20] {
        writeln!(src, "#define GENERATE_SEED() SENDER()").unwrap();
    } else if config.chain_id.is_some() && config.calling_address == [0u8; 20] {
        writeln!(src, "#define GENERATE_SEED() XCHAIN()").unwrap();
    } else {
        writeln!(src, "#define GENERATE_SEED() RANDOM()").unwrap();
    }

    match &config.reward {
        RewardVariant::LeadingZeros {
            leading_zeros_threshold,
        } => {
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasLeading(digest)").unwrap();
        }
        RewardVariant::Matching { pattern } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define PATTERN() \"{pattern}\"").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() isMatching(digest)").unwrap();
        }
    };

    let init_code_hash = match config.variant {
        CreateXVariant::Create2 { init_code_hash } => {
            writeln!(src, "#define CREATE3()").unwrap();
            init_code_hash
        }
        CreateXVariant::Create3 => {
            writeln!(src, "#define CREATE3() RUN_CREATE3()").unwrap();
            PROXY_CHILD_CODEHASH
        }
    };

    let caller = config.calling_address.iter();
    let chain_id = config
        .chain_id
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, x)| (i + 20, x));
    caller.enumerate().chain(chain_id).for_each(|(i, x)| {
        writeln!(src, "#define S1_{} {}u", i + 12, x).unwrap();
    });

    let factory = config.factory_address.iter();
    let hash = init_code_hash.iter();
    let hash = hash.enumerate().map(|(i, x)| (i + 52, x));

    for (i, x) in factory.enumerate().chain(hash) {
        writeln!(src, "#define S2_{} {}u", i + 1, x).unwrap();
    }

    src.push_str(KERNEL_SRC);

    src
}
