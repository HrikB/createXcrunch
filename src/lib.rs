use alloy_primitives::{hex, Address, FixedBytes};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use console::Term;
use fs4::FileExt;
use itertools::chain;
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

pub enum CreateXVariant {
    Create2 { init_code_hash: [u8; 32] },
    Create3,
}

pub enum RewardVariant {
    LeadingZeros {
        zeros_threshold: u8,
    },
    TotalZeros {
        zeros_threshold: u8,
    },
    LeadingAndTotalZeros {
        leading_zeros_threshold: u8,
        total_zeros_threshold: u8,
    },
    LeadingOrTotalZeros {
        leading_zeros_threshold: u8,
        total_zeros_threshold: u8,
    },
    Matching {
        pattern: Box<str>,
    },
}

pub enum SaltVariant {
    CrosschainSender {
        chain_id: [u8; 32],
        calling_address: [u8; 20],
    },
    Crosschain {
        chain_id: [u8; 32],
    },
    Sender {
        calling_address: [u8; 20],
    },
    Random,
}

pub struct Config<'a> {
    pub gpu_device: u8,
    pub factory_address: [u8; 20],
    pub salt_variant: SaltVariant,
    pub create_variant: CreateXVariant,
    pub reward: RewardVariant,
    pub output: &'a str,
}

impl<'a> Config<'a> {
    pub fn new(
        gpu_device: u8,
        factory_address_str: &str,
        calling_address_str: Option<&str>,
        chain_id: Option<u64>,
        init_code_hash: Option<&str>,
        reward: RewardVariant,
        output: &'a str,
    ) -> Result<Self, &'static str> {
        // convert main arguments from hex string to vector of bytes
        let factory_address_vec =
            hex::decode(factory_address_str).expect("could not decode factory address argument");
        let calling_address_vec = calling_address_str.map(|calling_address| {
            hex::decode(calling_address).expect("could not decode calling address argument")
        });
        let init_code_hash_vec = init_code_hash.map(|init_code_hash| {
            hex::decode(init_code_hash).expect("could not decode init code hash argument")
        });

        // convert from vector to fixed array
        let factory_address = TryInto::<[u8; 20]>::try_into(factory_address_vec)
            .expect("invalid length for factory address argument");
        let calling_address = calling_address_vec.map(|calling_address_vec| {
            TryInto::<[u8; 20]>::try_into(calling_address_vec)
                .expect("invalid length for calling address argument")
        });
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

        match &reward {
            RewardVariant::LeadingZeros { zeros_threshold }
            | RewardVariant::TotalZeros { zeros_threshold } => {
                validate_zeros_threshold(zeros_threshold)?;
            }
            RewardVariant::LeadingOrTotalZeros {
                leading_zeros_threshold,
                total_zeros_threshold,
            }
            | RewardVariant::LeadingAndTotalZeros {
                leading_zeros_threshold,
                total_zeros_threshold,
            } => {
                validate_zeros_threshold(leading_zeros_threshold)?;
                validate_zeros_threshold(total_zeros_threshold)?;
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

        fn validate_zeros_threshold(threhsold: &u8) -> Result<(), &'static str> {
            if threhsold == &0u8 {
                return Err("threshold must be greater than 0");
            }
            if threhsold > &20u8 {
                return Err("threshold must be less than 20");
            }

            Ok(())
        }

        let salt_variant = match (chain_id, calling_address) {
            (Some(chain_id), Some(calling_address)) if calling_address != [0u8; 20] => {
                SaltVariant::CrosschainSender {
                    chain_id,
                    calling_address,
                }
            }
            (Some(chain_id), None) => SaltVariant::Crosschain { chain_id },
            (None, Some(calling_address)) if calling_address != [0u8; 20] => {
                SaltVariant::Sender { calling_address }
            }
            _ => SaltVariant::Random,
        };

        if factory_address_str.chars().any(|c| c.is_uppercase()) {
            let factory_address_str = match factory_address_str.strip_prefix("0x") {
                Some(_) => factory_address_str.to_string(),
                None => format!("0x{}", factory_address_str),
            };
            match Address::parse_checksummed(factory_address_str, None) {
                Ok(_) => {}
                Err(_) => {
                    return Err("factory address uses invalid checksum");
                }
            }
        }

        if calling_address.is_some() {
            let calling_address_str = calling_address_str.unwrap();
            if calling_address_str.chars().any(|c| c.is_uppercase()) {
                let calling_address_str = match calling_address_str.strip_prefix("0x") {
                    Some(_) => calling_address_str.to_string(),
                    None => format!("0x{}", calling_address_str),
                };
                match Address::parse_checksummed(calling_address_str, None) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err("caller address uses invalid checksum");
                    }
                }
            };
        };

        Ok(Self {
            gpu_device,
            factory_address,
            salt_variant,
            create_variant,
            reward,
            output,
        })
    }
}

/// Adapted from https://github.com/0age/create2crunch
///
pub fn gpu(config: Config) -> ocl::Result<()> {
    println!(
        "Setting up OpenCL miner using device {}...",
        config.gpu_device
    );

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
                    CreateXVariant::Create2 { init_code_hash: _ } => "Create2",
                    CreateXVariant::Create3 {} => "Create3",
                };

                // display information about the current search criteria
                term.write_line(&format!(
                    "current search space: {}xxxxxxxx{:06x}\t\t\
                     threshold: mining for {} address {}",
                    hex::encode(salt),
                    // Only the first 3 bytes are used
                    BigEndian::read_u64(&view_buf) >> 8,
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

        // get the address that results from the hash
        let address = solutions[1]
            .to_be_bytes()
            .into_iter()
            .chain(solutions[2].to_be_bytes())
            .chain(solutions[3].to_be_bytes()[..4].to_vec())
            .collect::<Vec<u8>>();

        // count total and leading zero bytes
        let mut total = 0;
        let mut leading = 0;
        for (i, &b) in address.iter().enumerate() {
            if b == 0 {
                total += 1;
            } else if leading == 0 {
                // set leading on finding non-zero byte
                leading = i;
            }
        }

        let output = format!("0x{} => 0x{}", hex::encode(salt), hex::encode(address),);

        let show = format!("{output} ({leading} / {total})");
        match config.reward {
            RewardVariant::Matching { pattern: _ } => {
                found_list.push(output.to_string());
            }
            _ => {
                found_list.push(show);
            }
        }

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
        .open(config.output)
        .unwrap_or_else(|_| panic!("Could not create or open {} file.", config.output))
}

/// Creates the OpenCL kernel source code by populating the template with the
/// values from the Config object.
pub fn mk_kernel_src(config: &Config) -> String {
    let mut src = String::with_capacity(2048 + KERNEL_SRC.len());

    let (caller, chain_id) = match config.salt_variant {
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        } => {
            writeln!(src, "#define GENERATE_SEED() SENDER_XCHAIN()").unwrap();
            (calling_address, Some(chain_id))
        }
        SaltVariant::Crosschain { chain_id } => {
            writeln!(src, "#define GENERATE_SEED() XCHAIN()").unwrap();
            ([0u8; 20], Some(chain_id))
        }
        SaltVariant::Sender { calling_address } => {
            writeln!(src, "#define GENERATE_SEED() SENDER()").unwrap();
            (calling_address, None)
        }
        SaltVariant::Random => {
            writeln!(src, "#define GENERATE_SEED() RANDOM()").unwrap();
            ([0u8; 20], None)
        }
    };

    match &config.reward {
        RewardVariant::LeadingZeros { zeros_threshold } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "#define LEADING_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasLeading(digest)").unwrap();
        }
        RewardVariant::TotalZeros { zeros_threshold } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasTotal(digest)").unwrap();
        }
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() hasLeading(digest) && hasTotal(digest)"
            )
            .unwrap();
        }
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define PATTERN() 0").unwrap();
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() hasLeading(digest) || hasTotal(digest)"
            )
            .unwrap();
        }
        RewardVariant::Matching { pattern } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define PATTERN() \"{pattern}\"").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() isMatching(digest)").unwrap();
        }
    };

    let init_code_hash = match config.create_variant {
        CreateXVariant::Create2 { init_code_hash } => {
            writeln!(src, "#define CREATE3()").unwrap();
            init_code_hash
        }
        CreateXVariant::Create3 => {
            writeln!(src, "#define CREATE3() RUN_CREATE3()").unwrap();
            PROXY_CHILD_CODEHASH
        }
    };

    let caller = caller.iter();
    let chain_id = chain_id
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
