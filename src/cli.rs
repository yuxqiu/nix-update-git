use clap::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(name = "nix-update-git")]
#[command(version = concat!("v", env!("CARGO_PKG_VERSION"), "-", env!("GIT_HASH")))]
#[command(about = "Update git references in Nix flake files and Nix expressions")]
pub struct Cli {
    #[arg(value_hint = clap::ValueHint::FilePath)]
    pub files: Vec<std::path::PathBuf>,

    #[arg(short, long, help = "Check without making changes (default)")]
    pub check: bool,

    #[arg(short, long, help = "Perform updates")]
    pub update: bool,

    #[arg(short, long, help = "Confirm each update")]
    pub interactive: bool,

    #[arg(short, long, help = "Enable verbose output")]
    pub verbose: bool,

    #[arg(
        long,
        value_name = "FORMAT",
        default_value = "text",
        help = "Output format: text or json"
    )]
    pub format: OutputFormat,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(format!(
                "unknown output format: {s} (expected 'text' or 'json')"
            )),
        }
    }
}
