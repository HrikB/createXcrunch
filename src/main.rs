use clap::{command, Arg, ArgGroup, Args, Parser, Subcommand};
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
            let init_code_hash = args.init_code_hash;
            let reward = match (args.cli_args.zeros, args.cli_args.pattern) {
                (Some(zeros), None) => RewardVariant::LeadingZeros {
                    leading_zeros_threshold: zeros
                        .parse::<u8>()
                        .expect("Leading zeros threshold must be a number"),
                },
                (None, Some(pattern)) => RewardVariant::Matching {
                    pattern: pattern.to_string(),
                },
                _ => unreachable!(),
            };
            let output = args.cli_args.output;

            let _ = match Config::new(
                &gpu_device_id,
                &factory,
                &caller,
                chain_id.as_ref(),
                Some(&init_code_hash),
                reward,
                &output,
            ) {
                Ok(config) => gpu(config),
                Err(e) => panic!("{}", e),
            };
        }
        Commands::Create3(args) => {
            let gpu_device_id = args.gpu_device_id;
            let factory = args.factory;
            let caller = args.caller;
            let chain_id = args.chain_id;
            let reward = match (args.zeros, args.pattern) {
                (Some(zeros), None) => RewardVariant::LeadingZeros {
                    leading_zeros_threshold: zeros
                        .parse::<u8>()
                        .expect("Leading zeros threshold must be a number"),
                },
                (None, Some(pattern)) => RewardVariant::Matching {
                    pattern: pattern.to_string(),
                },
                _ => unreachable!(),
            };
            let output = args.output;

            let _ = match Config::new(
                &gpu_device_id,
                &factory,
                &caller,
                chain_id.as_ref(),
                None,
                reward,
                &output,
            ) {
                Ok(config) => gpu(config),
                Err(e) => panic!("{}", e),
            };
        }
    }
}
