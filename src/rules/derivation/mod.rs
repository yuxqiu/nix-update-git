mod core;

pub use core::DerivationRule;

pub const OWNED_FUNC_NAMES: &[&str] = &[
    "mkDerivation",
    "buildRustPackage",
    "buildGoModule",
    "buildGoPackage",
    "buildPythonPackage",
    "buildPythonApplication",
    "buildDunePackage",
    "buildNpmPackage",
    "buildMixPackage",
    "buildRebar3Release",
    "buildGem",
    "buildHaskellPackage",
    "mkHaskellPackage",
    "buildEmscriptenPackage",
];

pub fn mk_derivation_rule() -> DerivationRule {
    DerivationRule::new("mk-derivation", vec!["mkDerivation".to_string()])
}

pub fn build_rust_package_rule() -> DerivationRule {
    DerivationRule::new("build-rust-package", vec!["buildRustPackage".to_string()])
}

pub fn build_go_module_rule() -> DerivationRule {
    DerivationRule::new(
        "build-go-module",
        vec!["buildGoModule".to_string(), "buildGoPackage".to_string()],
    )
}

pub fn build_python_package_rule() -> DerivationRule {
    DerivationRule::new(
        "build-python-package",
        vec![
            "buildPythonPackage".to_string(),
            "buildPythonApplication".to_string(),
        ],
    )
}

pub fn build_dune_package_rule() -> DerivationRule {
    DerivationRule::new("build-dune-package", vec!["buildDunePackage".to_string()])
}

pub fn build_npm_package_rule() -> DerivationRule {
    DerivationRule::new("build-npm-package", vec!["buildNpmPackage".to_string()])
}

pub fn build_mix_package_rule() -> DerivationRule {
    DerivationRule::new("build-mix-package", vec!["buildMixPackage".to_string()])
}

pub fn build_rebar3_release_rule() -> DerivationRule {
    DerivationRule::new(
        "build-rebar3-release",
        vec!["buildRebar3Release".to_string()],
    )
}

pub fn build_gem_rule() -> DerivationRule {
    DerivationRule::new("build-gem", vec!["buildGem".to_string()])
}

pub fn build_haskell_package_rule() -> DerivationRule {
    DerivationRule::new(
        "build-haskell-package",
        vec![
            "buildHaskellPackage".to_string(),
            "mkHaskellPackage".to_string(),
        ],
    )
}

pub fn build_emscripten_package_rule() -> DerivationRule {
    DerivationRule::new(
        "build-emscripten-package",
        vec!["buildEmscriptenPackage".to_string()],
    )
}
