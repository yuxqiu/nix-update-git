use clap::{Parser, ValueHint};

#[derive(Parser, Debug)]
#[command(name = "nix-update-git")]
#[command(version = "0.1.0")]
#[command(about = "Update git references in Nix flake files and Nix expressions", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(value_hint = ValueHint::FilePath)]
    pub files: Vec<std::path::PathBuf>,

    #[arg(short, long, help = "Check without making changes (default)")]
    pub check: bool,

    #[arg(short, long, help = "Perform updates")]
    pub update: bool,

    #[arg(short, long, help = "Confirm each update")]
    pub interactive: bool,

    #[arg(
        short,
        long,
        help = "Number of parallel update jobs",
        default_value = "4"
    )]
    pub jobs: usize,
}

#[derive(Parser, Debug)]
pub enum Command {
    #[command(about = "Print AST of Nix files")]
    Ast {
        #[arg(value_hint = ValueHint::FilePath)]
        files: Vec<std::path::PathBuf>,
    },
}
