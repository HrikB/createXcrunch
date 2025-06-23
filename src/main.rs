use clap::Parser;
use createxcrunch::{
    cli::{Cli, Commands},
    gpu, Config, RewardVariant,
};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create2(args) => {
            let gpu_device_id = args.cli_args.gpu_device_id;
            let factory = args.cli_args.factory;
            let caller = args.cli_args.caller;
            let chain_id = args.cli_args.chain_id;
            let use_metal = args.cli_args.use_metal;
            let init_code_hash = args.init_code_hash;
            let reward = match (
                args.cli_args.zeros,
                args.cli_args.total,
                args.cli_args.either,
                args.cli_args.pattern,
            ) {
                (Some(zeros), None, false, None) => RewardVariant::LeadingZeros {
                    zeros_threshold: zeros,
                },
                (None, Some(total), false, None) => RewardVariant::TotalZeros {
                    zeros_threshold: total,
                },
                (Some(zeros), Some(total), false, None) => RewardVariant::LeadingAndTotalZeros {
                    leading_zeros_threshold: zeros,
                    total_zeros_threshold: total,
                },
                (Some(zeros), Some(total), true, None) => RewardVariant::LeadingOrTotalZeros {
                    leading_zeros_threshold: zeros,
                    total_zeros_threshold: total,
                },
                (None, None, false, Some(pattern)) => {
                    let pattern = pattern
                        .strip_prefix("0x")
                        .unwrap_or(&pattern)
                        .to_owned()
                        .into_boxed_str();
                    RewardVariant::Matching { pattern: pattern }
                }
                _ => unreachable!(),
            };
            let output = args.cli_args.output;

            match Config::new(
                gpu_device_id,
                &factory,
                caller.as_deref(),
                chain_id,
                Some(&init_code_hash),
                reward,
                &output,
                use_metal,
            ) {
                Ok(config) => {
                    #[cfg(target_os = "macos")]
                    {
                        if config.use_metal {
                            match createxcrunch::metal_gpu::gpu_metal(config) {
                                Ok(_) => (),
                                Err(e) => panic!("{}", e),
                            }
                        } else {
                            match gpu(config) {
                                Ok(_) => (),
                                Err(e) => panic!("{}", e),
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        if use_metal {
                            panic!("Metal is only available on macOS");
                        }
                        match gpu(config) {
                            Ok(_) => (),
                            Err(e) => panic!("{}", e),
                        }
                    }
                },
                Err(e) => panic!("{}", e),
            };
        }
        Commands::Create3(args) => {
            let gpu_device_id = args.gpu_device_id;
            let factory = args.factory;
            let caller = args.caller;
            let chain_id = args.chain_id;
            let use_metal = args.use_metal;
            let reward = match (args.zeros, args.total, args.either, args.pattern) {
                (Some(zeros), None, false, None) => RewardVariant::LeadingZeros {
                    zeros_threshold: zeros,
                },
                (None, Some(total), false, None) => RewardVariant::TotalZeros {
                    zeros_threshold: total,
                },
                (Some(zeros), Some(total), false, None) => RewardVariant::LeadingAndTotalZeros {
                    leading_zeros_threshold: zeros,
                    total_zeros_threshold: total,
                },
                (Some(zeros), Some(total), true, None) => RewardVariant::LeadingOrTotalZeros {
                    leading_zeros_threshold: zeros,
                    total_zeros_threshold: total,
                },
                (None, None, false, Some(pattern)) => {
                    let pattern = pattern
                        .strip_prefix("0x")
                        .unwrap_or(&pattern)
                        .to_owned()
                        .into_boxed_str();
                    RewardVariant::Matching { pattern }
                }
                _ => unreachable!(),
            };
            let output = args.output;

            match Config::new(
                gpu_device_id,
                &factory,
                caller.as_deref(),
                chain_id,
                None,
                reward,
                &output,
                use_metal,
            ) {
                Ok(config) => {
                    #[cfg(target_os = "macos")]
                    {
                        if config.use_metal {
                            match createxcrunch::metal_gpu::gpu_metal(config) {
                                Ok(_) => (),
                                Err(e) => panic!("{}", e),
                            }
                        } else {
                            match gpu(config) {
                                Ok(_) => (),
                                Err(e) => panic!("{}", e),
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        if use_metal {
                            panic!("Metal is only available on macOS");
                        }
                        match gpu(config) {
                            Ok(_) => (),
                            Err(e) => panic!("{}", e),
                        }
                    }
                },
                Err(e) => panic!("{}", e),
            };
        }
    }
}
