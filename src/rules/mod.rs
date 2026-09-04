pub mod derivation;
pub mod fetcher;
pub mod flake_input;
pub mod traits;

pub use derivation::{
    DerivationRule, OWNED_FUNC_NAMES, build_dune_package_rule, build_emscripten_package_rule,
    build_gem_rule, build_go_module_rule, build_haskell_package_rule, build_mix_package_rule,
    build_npm_package_rule, build_python_package_rule, build_rebar3_release_rule,
    build_rust_package_rule, build_vim_plugin_rule, mk_derivation_rule,
};
pub use fetcher::FetcherRule;
pub use flake_input::FlakeInputRule;
pub use traits::{CheckResult, CheckWarning, RuleRegistry, Update, UpdateGroup, UpdateRule};

use crate::cli::RuleName;

/// Builds a `RuleRegistry` containing the rules enabled by `rules`
/// (interpreting `RuleName::All` as "every rule"). Shared by the CLI and by
/// tests that need the same default rule set without going through a
/// subprocess.
pub fn build_registry(rules: &[RuleName]) -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    let rule_enabled = |name: &str| rules.iter().any(|r| r.is_enabled(name));

    if rule_enabled("flake") {
        registry.register(FlakeInputRule);
    }
    if rule_enabled("fetcher") {
        registry.register(FetcherRule);
    }
    if rule_enabled("mk-derivation") {
        registry.register(mk_derivation_rule());
    }
    if rule_enabled("build-vim-plugin") {
        registry.register(build_vim_plugin_rule());
    }
    if rule_enabled("build-rust-package") {
        registry.register(build_rust_package_rule());
    }
    if rule_enabled("build-go-module") {
        registry.register(build_go_module_rule());
    }
    if rule_enabled("build-python-package") {
        registry.register(build_python_package_rule());
    }
    if rule_enabled("build-dune-package") {
        registry.register(build_dune_package_rule());
    }
    if rule_enabled("build-npm-package") {
        registry.register(build_npm_package_rule());
    }
    if rule_enabled("build-mix-package") {
        registry.register(build_mix_package_rule());
    }
    if rule_enabled("build-rebar3-release") {
        registry.register(build_rebar3_release_rule());
    }
    if rule_enabled("build-gem") {
        registry.register(build_gem_rule());
    }
    if rule_enabled("build-haskell-package") {
        registry.register(build_haskell_package_rule());
    }
    if rule_enabled("build-emscripten-package") {
        registry.register(build_emscripten_package_rule());
    }

    registry
}
