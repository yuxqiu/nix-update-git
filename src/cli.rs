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
    BuildRustPackage,
    BuildGoModule,
    BuildPythonPackage,
    BuildDunePackage,
    BuildNpmPackage,
    BuildMixPackage,
    BuildRebar3Release,
    BuildGem,
    BuildHaskellPackage,
    BuildEmscriptenPackage,
}

impl RuleName {
    pub fn rule_id(&self) -> &'static str {
        match self {
            RuleName::All => "all",
            RuleName::Fetcher => "fetcher",
            RuleName::Flake => "flake",
            RuleName::MkDerivation => "mk-derivation",
            RuleName::BuildRustPackage => "build-rust-package",
            RuleName::BuildGoModule => "build-go-module",
            RuleName::BuildPythonPackage => "build-python-package",
            RuleName::BuildDunePackage => "build-dune-package",
            RuleName::BuildNpmPackage => "build-npm-package",
            RuleName::BuildMixPackage => "build-mix-package",
            RuleName::BuildRebar3Release => "build-rebar3-release",
            RuleName::BuildGem => "build-gem",
            RuleName::BuildHaskellPackage => "build-haskell-package",
            RuleName::BuildEmscriptenPackage => "build-emscripten-package",
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        if matches!(self, RuleName::All) {
            return true;
        }
        self.rule_id() == name
    }
}

pub fn default_rules() -> Vec<RuleName> {
    vec![RuleName::Fetcher, RuleName::Flake, RuleName::MkDerivation]
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
        default_values_t = default_rules(),
        help = "Rules to enable"
    )]
    pub rules: Vec<RuleName>,
}
