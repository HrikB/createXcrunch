use alloy_primitives::hex::{decode, encode};
use alloy_primitives::FixedBytes;
use byteorder::{ByteOrder, LittleEndian};
use createxcrunch::{mk_kernel_src, Config, CreateXVariant, RewardVariant, SaltVariant};
use itertools::chain;
use ocl::{Buffer, Context, Device, MemFlags, Platform, ProQue, Program, Queue};
use rstest::*;

#[fixture]
fn try_nonce(
    #[default(SaltVariant::Random)] salt_variant: SaltVariant,
    #[default(CreateXVariant::Create3)] create_variant: CreateXVariant,
    #[default(RewardVariant::LeadingZeros { zeros_threshold: 1 })] reward: RewardVariant,
    #[default([0; 1])] nonce: [u32; 1],
) -> ocl::Result<String> {
    let config = Config {
        gpu_device: 0,
        // 0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed
        factory_address: [
            186, 94, 208, 153, 99, 61, 59, 49, 62, 77, 95, 123, 220, 19, 5, 211, 194, 139, 165, 237,
        ],
        salt_variant,
        create_variant,
        reward,
        // This field will be ignored for tests
        output: "output.txt",
    };
    // set up a platform to use
    let platform = Platform::new(ocl::core::default_platform()?);

    let device = Device::by_idx_wrap(platform, config.gpu_device as usize)?;

    // set up the context to use
    let context = Context::builder()
        .platform(platform)
        .devices(device)
        .build()?;

    let program = Program::builder()
        .devices(device)
        .src(mk_kernel_src(&config))
        .build(&context)?;

    // set up the queue to use
    let queue = Queue::new(&context, device, None)?;

    // set up the "proqueue" (or amalgamation of various elements) to use
    let ocl_pq = ProQue::new(context, queue, program, Some(1));

    // construct the 4-byte message to hash, leaving last 8 of salt empty
    let salt = FixedBytes::<4>::try_from(&[0u8; 4]).unwrap();

    // build a corresponding buffer for passing the message to the kernel
    let message_buffer = Buffer::builder()
        .queue(ocl_pq.queue().clone())
        .flags(MemFlags::new().read_only())
        .len(4)
        .copy_host_slice(&salt[..])
        .build()?;

    // reset nonce & create a buffer to view it in little-endian
    // for more uniformly distributed nonces, we shall initialize it to a random value
    let mut view_buf = [0; 8];

    LittleEndian::write_u64(&mut view_buf, (nonce[0] as u64) << 32);

    // build a corresponding buffer for passing the nonce to the kernel
    let nonce_buffer = Buffer::builder()
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

    let global_work_size = [1, 1, 1]; // This effectively sets get_global_id(0) to 0 for a single work item

    // enqueue the kernel
    unsafe {
        kern.cmd().global_work_size(global_work_size).enq()?;
    }

    // read the solutions from the device
    solutions_buffer.read(&mut solutions).enq()?;

    let solution = solutions[0];
    let solution = solution.to_le_bytes();

    println!("Solution: {:?}", solution);

    let mined_salt = chain!(salt, solution[..7].iter().copied());

    let salt: Vec<u8> = match config.salt_variant {
        SaltVariant::CrosschainSender {
            chain_id: _,
            calling_address,
        } => chain!(calling_address, [1u8], mined_salt).collect(),
        SaltVariant::Crosschain { chain_id: _ } => chain!([0u8; 20], [1u8], mined_salt).collect(),
        SaltVariant::Sender { calling_address } => {
            chain!(calling_address, [0u8], mined_salt).collect()
        }
        SaltVariant::Random => chain!(mined_salt, [0u8; 21]).collect(),
    };

    println!("Salt: {:?}", salt);

    // get the address that results from the hash
    let mut address = encode(
        solutions[1]
            .to_be_bytes()
            .into_iter()
            .chain(solutions[2].to_be_bytes())
            .chain(solutions[3].to_be_bytes()[..4].to_vec())
            .collect::<Vec<u8>>(),
    );

    address.insert_str(0, "0x");

    Ok(address)
}

#[rstest]
fn test_create3_random() {
    let address = try_nonce(
        SaltVariant::Random,
        CreateXVariant::Create3,
        RewardVariant::LeadingZeros { zeros_threshold: 1 },
        [61u32; 1],
    )
    .unwrap();

    assert_eq!("0x00945498be46467fee556bf2f2f3dcfbd1a6765a", address);

    let address = try_nonce(
        SaltVariant::Random,
        CreateXVariant::Create3,
        RewardVariant::TotalZeros { zeros_threshold: 2 },
        [357u32; 1],
    )
    .unwrap();

    assert_eq!("0x4c788c0e302910a2c95a000684d47d2d00591809", address);

    let address = try_nonce(
        SaltVariant::Random,
        CreateXVariant::Create3,
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [61u32; 1],
    )
    .unwrap();

    assert_eq!("0x00945498be46467fee556bf2f2f3dcfbd1a6765a", address);

    let address = try_nonce(
        SaltVariant::Random,
        CreateXVariant::Create3,
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 5,
            total_zeros_threshold: 2,
        },
        [357u32; 1],
    )
    .unwrap();

    assert_eq!("0x4c788c0e302910a2c95a000684d47d2d00591809", address);

    let address = try_nonce(
        SaltVariant::Random,
        CreateXVariant::Create3,
        RewardVariant::Matching {
            pattern: "bbXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
                .to_owned()
                .into_boxed_str(),
        },
        [87u32; 1],
    )
    .unwrap();

    assert_eq!("0xbb10c35fdadda68390f7f58b4378ad07826a5471", address);
}

#[rstest]
fn test_create3_caller() {
    let calling_address = string_to_addr_bytes("0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5");

    let address = try_nonce(
        SaltVariant::Sender { calling_address },
        CreateXVariant::Create3,
        RewardVariant::LeadingZeros { zeros_threshold: 1 },
        [66u32; 1],
    )
    .unwrap();

    assert_eq!("0x0060e8253a9f9b04d9126b79d77bd022a59e7f9a", address);

    let address = try_nonce(
        SaltVariant::Sender { calling_address },
        CreateXVariant::Create3,
        RewardVariant::TotalZeros { zeros_threshold: 2 },
        [1579u32; 1],
    )
    .unwrap();

    assert_eq!("0x00ebab0f93b64b8714006f13872816beca04ee88", address);

    let address = try_nonce(
        SaltVariant::Sender { calling_address },
        CreateXVariant::Create3,
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [66u32; 1],
    )
    .unwrap();

    assert_eq!("0x0060e8253a9f9b04d9126b79d77bd022a59e7f9a", address);

    let address = try_nonce(
        SaltVariant::Sender { calling_address },
        CreateXVariant::Create3,
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 5,
            total_zeros_threshold: 2,
        },
        [1579u32; 1],
    )
    .unwrap();

    assert_eq!("0x00ebab0f93b64b8714006f13872816beca04ee88", address);

    let address = try_nonce(
        SaltVariant::Sender { calling_address },
        CreateXVariant::Create3,
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [1579u32; 1],
    )
    .unwrap();

    assert_eq!("0x00ebab0f93b64b8714006f13872816beca04ee88", address);

    let address = try_nonce(
        SaltVariant::Sender { calling_address },
        CreateXVariant::Create3,
        RewardVariant::Matching {
            pattern: "bbXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
                .to_owned()
                .into_boxed_str(),
        },
        [152u32; 1],
    )
    .unwrap();

    assert_eq!("0xbb660249e599b0d9b21015fa7ebd97fd78141737", address);
}

#[rstest]
fn test_create2_crosschain() {
    let mut chain_id = [0u8; 32];
    chain_id[31] = 1;

    let init_code_hash = [0u8; 32];

    let address = try_nonce(
        SaltVariant::Crosschain { chain_id },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingZeros { zeros_threshold: 1 },
        [126u32; 1],
    )
    .unwrap();

    assert_eq!("0x006b3047dc49181a8cf360813681ab36246c5b85", address);

    let address = try_nonce(
        SaltVariant::Crosschain { chain_id },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::TotalZeros { zeros_threshold: 2 },
        [746u32; 1],
    )
    .unwrap();

    assert_eq!("0xb62e9ad35c5c7865a6090a00ba5a0074b2100947", address);

    let address = try_nonce(
        SaltVariant::Crosschain { chain_id },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [126u32; 1],
    )
    .unwrap();

    assert_eq!("0x006b3047dc49181a8cf360813681ab36246c5b85", address);

    let address = try_nonce(
        SaltVariant::Crosschain { chain_id },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 5,
            total_zeros_threshold: 2,
        },
        [746u32; 1],
    )
    .unwrap();

    assert_eq!("0xb62e9ad35c5c7865a6090a00ba5a0074b2100947", address);

    let address = try_nonce(
        SaltVariant::Crosschain { chain_id },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [2091u32; 1],
    )
    .unwrap();

    assert_eq!("0x00005d7c0b23ffc4036554dea00ecbb6b5f82ba0", address);

    let address = try_nonce(
        SaltVariant::Crosschain { chain_id },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::Matching {
            pattern: "bbXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
                .to_owned()
                .into_boxed_str(),
        },
        [45u32; 1],
    )
    .unwrap();

    assert_eq!("0xbbf5e44c1302d0228d95ff916ee5aa3ee39334bb", address);
}

#[rstest]
fn test_create2_crosschain_caller() {
    let mut chain_id = [0u8; 32];
    chain_id[31] = 1;

    let init_code_hash = [0u8; 32];

    let calling_address = string_to_addr_bytes("0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5");

    let address = try_nonce(
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingZeros { zeros_threshold: 1 },
        [343u32; 1],
    )
    .unwrap();

    assert_eq!("0x00abb8aa06547cd6c2f4cf447448ba19f18f7155", address);

    let address = try_nonce(
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::TotalZeros { zeros_threshold: 2 },
        [487u32; 1],
    )
    .unwrap();

    assert_eq!("0xa3827c31ec59d70000e091d390670750f3b0e804", address);

    let address = try_nonce(
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [343u32; 1],
    )
    .unwrap();

    assert_eq!("0x00abb8aa06547cd6c2f4cf447448ba19f18f7155", address);

    let address = try_nonce(
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: 5,
            total_zeros_threshold: 2,
        },
        [487u32; 1],
    )
    .unwrap();

    assert_eq!("0xa3827c31ec59d70000e091d390670750f3b0e804", address);

    let address = try_nonce(
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold: 1,
            total_zeros_threshold: 2,
        },
        [759u32; 1],
    )
    .unwrap();

    assert_eq!("0x004e286d958dffee00dfdccfd438483516fc0c93", address);

    let address = try_nonce(
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        },
        CreateXVariant::Create2 { init_code_hash },
        RewardVariant::Matching {
            pattern: "bbXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
                .to_owned()
                .into_boxed_str(),
        },
        [50u32; 1],
    )
    .unwrap();

    assert_eq!("0xbbfaecabdd12e01f3a4ce699095ab6dbd1a62b1c", address);
}

fn string_to_addr_bytes(s: &str) -> [u8; 20] {
    let mut addr = [0u8; 20];
    let s = s.trim_start_matches("0x");
    let bytes = decode(s).unwrap();
    addr.copy_from_slice(&bytes);
    addr
}
