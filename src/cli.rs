use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "nix-update-git")]
#[command(version = "0.1.0")]
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
}
