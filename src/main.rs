use clap::{command, Arg, ArgGroup};
use createxcrunch::{gpu, Config, RewardVariant};

fn main() {
    let args = [
        Arg::new("factory")
            .long("factory")
            .short('f')
            .default_value("0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed")
            .long_help("Set the factory address.")
            .help_heading("Crunching options"),
        Arg::new("gpu-device-id")
            .long("gpu")
            .short('g')
            .default_value("0")
            .long_help("Set the GPU device ID.")
            .help_heading("Crunching options"),
        Arg::new("caller")
            .long("caller")
            .short('c')
            .default_value("0x0000000000000000000000000000000000000000")
            .long_help("Set the caller address in hex format for a permissioned deployment.")
            .help_heading("Crunching options"),
        Arg::new("chain-id")
            .long("crosschain")
            .short('x')
            .visible_alias("crp")
            .long_help("Set whether or not to enable crosschain deployment protection.")
            .help_heading("Crunching options"),
        Arg::new("zeros")
            .long("leading")
            .short('z')
            .long_help("Minimum number of leading zeros. Cannot be used in combination with -m.\n\nExample: -z 4.")       
            .help_heading("Crunching options"),
        Arg::new("pattern")
            .long("matching")
            .short('m')
            .long_help("Matching pattern for the contract address. Cannot be used in combination with -z.\n\nExample: -m ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed.")
            .help_heading("Crunching options"),
        Arg::new("output")
            .long("output")
            .short('o')
            .default_value("output.txt")
            .long_help("Output file name.")
            .help_heading("Output options")
    ];

    // Group enforces mutual exclusivity
    let pattern_group = ArgGroup::new("mining-pattern")
        .arg("zeros")
        .arg("pattern")
        .required(true);

    let matches = command!()
        .subcommand(
            command!("create3")
                .args(&args)
                .about("Mine for a CREATE3 deployment address.")
                .group(&pattern_group),
        )
        .subcommand(
            command!("create2")
                .arg(
                    Arg::new("init-code-hash")
                        .long("code-hash")
                        .visible_alias("ch")
                        .long_help("Set the init code hash in hex format.")
                        .help_heading("Crunching options")
                        .required(true),
                )
                .args(&args)
                .about("Mine for a CREATE2 deployment address.")
                .group(&pattern_group),
        )
        .arg_required_else_help(true)
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("create3") {
        let (zeros, pattern) = (
            matches.get_one::<String>("zeros"),
            matches.get_one::<String>("pattern"),
        );

        let gpu_device_id = matches.get_one::<String>("gpu-device-id").unwrap();
        let factory = matches.get_one::<String>("factory").unwrap();
        let caller = matches.get_one::<String>("caller").unwrap();
        let chain_id = matches.get_one::<String>("chain-id");
        let reward = match (zeros, pattern) {
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
        let output = matches.get_one::<String>("output").unwrap();

        let _ = match Config::new(
            gpu_device_id,
            factory,
            caller,
            chain_id,
            None,
            reward,
            output,
        ) {
            Ok(config) => gpu(config),
            Err(e) => panic!("{}", e),
        };
    }
}
