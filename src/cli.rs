use clap::{command, ArgAction, ArgGroup, Args, Parser, Subcommand};

#[derive(Parser)]
#[command(arg_required_else_help = true)]
#[clap(name = "createXcrunch", version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
#[clap(group = ArgGroup::new("search-criteria").multiple(true).required(true))]
#[clap(group = ArgGroup::new("zeros-threshold"))]
pub struct CliArgs {
    #[arg(
        id = "factory",
        long,
        short,
        default_value = "0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed",
        long_help = "Set the factory address.",
        help_heading = "Crunching options"
    )]
    pub factory: String,

    #[arg(
        id = "gpu-device-id",
        long,
        short,
        default_value = "0",
        long_help = "Set the GPU device ID.",
        help_heading = "Crunching options"
    )]
    pub gpu_device_id: u8,

    #[arg(
        id = "caller",
        long,
        short,
        long_help = "Set the caller address in hex format for a permissioned deployment.",
        help_heading = "Crunching options"
    )]
    pub caller: Option<String>,

    #[arg(
        id = "chain-id",
        long = "crosschain",
        short = 'x',
        long_help = "Set whether or not to enable crosschain deployment protection.",
        help_heading = "Crunching options",
        visible_alias = "crp"
    )]
    pub chain_id: Option<u64>,

    #[arg(
        id = "zeros",
        long = "leading",
        short = 'z',
        group = "search-criteria",
        long_help = "Minimum number of leading zero bytes. Cannot be used in combination with --matching.\n\nExample: --leading 4.",
        help_heading = "Crunching options"
    )]
    pub zeros: Option<u8>,

    #[arg(
        id = "total",
        long = "total",
        short = 't',
        group = "search-criteria",
        long_help = "Total number of zero bytes. If used in conjunction with --leading, search criteria will be both thresholds. Pass --either to search for either threshold.\n\nExample: --total 32.",
        help_heading = "Crunching options"
    )]
    pub total: Option<u8>,

    #[arg(
        id = "either",
        long = "either",
        long_help = "Search for either threshold. Must be used with --leading and --total.",
        requires_all = &["zeros", "total"],
        action = ArgAction::SetTrue,
        help_heading = "Crunching options"
    )]
    pub either: bool,

    #[arg(
        id = "pattern",
        long = "matching",
        short = 'm',
        group = "search-criteria",
        long_help = "Matching pattern for the contract address. Cannot be used in combination with --leading.\n\nExample: --matching ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed.",
        help_heading = "Crunching options",
        conflicts_with_all = &["zeros", "total"]
    )]
    pub pattern: Option<Box<str>>,

    #[arg(
        id = "output",
        long,
        short,
        default_value = "output.txt",
        long_help = "Output file name.",
        help_heading = "Output options"
    )]
    pub output: String,
}

#[derive(Args)]
pub struct Create2Args {
    #[clap(flatten)]
    pub cli_args: CliArgs,

    #[arg(
        long = "code-hash",
        visible_alias = "ch",
        long_help = "Set the init code hash in hex format.",
        help_heading = "Crunching options",
        required = true,
        visible_alias = "ch"
    )]
    pub init_code_hash: String,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Mine for a CREATE3 deployment address.")]
    Create3(CliArgs),
    #[command(about = "Mine for a CREATE2 deployment address.")]
    Create2(Create2Args),
}
