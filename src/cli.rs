use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RuleName {
    All,
    Fetcher,
    Flake,
    MkDerivation,
}

impl RuleName {
    pub fn is_enabled(&self, name: &str) -> bool {
        match self {
            RuleName::All => true,
            RuleName::Fetcher => name == "fetcher",
            RuleName::Flake => name == "flake_input",
            RuleName::MkDerivation => name == "mk_derivation",
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "nix-update-git")]
#[command(version = concat!("v", env!("CARGO_PKG_VERSION"), "-", env!("GIT_HASH")))]
#[command(about = "Update git references in Nix flake files and Nix expressions")]
pub struct Cli {
    #[arg(value_hint = clap::ValueHint::AnyPath, help = "Nix files or directories containing .nix files")]
    pub files_or_directories: Vec<std::path::PathBuf>,

    #[arg(short, long, help = "Check without making changes (default)")]
    pub check: bool,

    #[arg(short, long, help = "Perform updates")]
    pub update: bool,

    #[arg(short, long, help = "Confirm each update")]
    pub interactive: bool,

    #[arg(short, long, help = "Enable verbose output")]
    pub verbose: bool,

    #[arg(long, value_enum, default_value = "text", help = "Output format")]
    pub format: OutputFormat,

    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 4,
        help = "Number of parallel file processing jobs"
    )]
    pub jobs: usize,

    #[arg(
        short,
        long,
        value_enum,
        num_args = 1..,
        default_values_t = [RuleName::Fetcher, RuleName::Flake, RuleName::MkDerivation],
        help = "Rules to enable"
    )]
    pub rules: Vec<RuleName>,
}
