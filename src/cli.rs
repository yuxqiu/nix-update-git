use clap::Parser;
use clap::builder::PossibleValuesParser;

use crate::rules::{default_rule_ids, rule_value_names};

#[derive(Parser, Debug)]
#[command(name = "nix-update-git")]
#[command(version = concat!("v", env!("CARGO_PKG_VERSION"), "-", env!("GIT_HASH")))]
#[command(about = "Update git references in Nix flake files and Nix expressions")]
// Each bool is an independent command-line flag (clap's natural
// representation for `--check`/`--update`/`--interactive`/`--verbose`), not
// state that would benefit from an enum.
#[allow(clippy::struct_excessive_bools)]
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
        value_parser = PossibleValuesParser::new(rule_value_names()),
        num_args = 1..,
        default_values_t = default_rule_ids(),
        help = "Rules to enable"
    )]
    pub rules: Vec<String>,
}
