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

/// One entry in the rule registry: an id the CLI/tests refer to it by,
/// whether it's on by default, and how to build it. Adding a rule means
/// adding one entry here — `main.rs`, `cli.rs`, and the snapshot test
/// harness all consume `RULES` generically.
pub struct RuleDescriptor {
    pub id: &'static str,
    pub default_enabled: bool,
    pub build: fn() -> Box<dyn UpdateRule>,
}

pub const RULES: &[RuleDescriptor] = &[
    RuleDescriptor {
        id: "flake",
        default_enabled: true,
        build: || Box::new(FlakeInputRule),
    },
    RuleDescriptor {
        id: "fetcher",
        default_enabled: true,
        build: || Box::new(FetcherRule),
    },
    RuleDescriptor {
        id: "mk-derivation",
        default_enabled: true,
        build: || Box::new(mk_derivation_rule()),
    },
    RuleDescriptor {
        id: "build-vim-plugin",
        default_enabled: true,
        build: || Box::new(build_vim_plugin_rule()),
    },
    RuleDescriptor {
        id: "build-rust-package",
        default_enabled: false,
        build: || Box::new(build_rust_package_rule()),
    },
    RuleDescriptor {
        id: "build-go-module",
        default_enabled: false,
        build: || Box::new(build_go_module_rule()),
    },
    RuleDescriptor {
        id: "build-python-package",
        default_enabled: false,
        build: || Box::new(build_python_package_rule()),
    },
    RuleDescriptor {
        id: "build-dune-package",
        default_enabled: false,
        build: || Box::new(build_dune_package_rule()),
    },
    RuleDescriptor {
        id: "build-npm-package",
        default_enabled: false,
        build: || Box::new(build_npm_package_rule()),
    },
    RuleDescriptor {
        id: "build-mix-package",
        default_enabled: false,
        build: || Box::new(build_mix_package_rule()),
    },
    RuleDescriptor {
        id: "build-rebar3-release",
        default_enabled: false,
        build: || Box::new(build_rebar3_release_rule()),
    },
    RuleDescriptor {
        id: "build-gem",
        default_enabled: false,
        build: || Box::new(build_gem_rule()),
    },
    RuleDescriptor {
        id: "build-haskell-package",
        default_enabled: false,
        build: || Box::new(build_haskell_package_rule()),
    },
    RuleDescriptor {
        id: "build-emscripten-package",
        default_enabled: false,
        build: || Box::new(build_emscripten_package_rule()),
    },
];

/// The `all` sentinel accepted by `--rules`, meaning "every registered rule".
pub const ALL_RULES_SENTINEL: &str = "all";

/// Rule ids enabled by default (used as `--rules`' default value).
pub fn default_rule_ids() -> Vec<String> {
    RULES
        .iter()
        .filter(|r| r.default_enabled)
        .map(|r| r.id.to_string())
        .collect()
}

/// All valid `--rules` values: every registered rule id, plus `all`.
pub fn rule_value_names() -> Vec<&'static str> {
    RULES
        .iter()
        .map(|r| r.id)
        .chain(std::iter::once(ALL_RULES_SENTINEL))
        .collect()
}

/// Builds a `RuleRegistry` containing the rules named in `requested`
/// (`"all"` enables every registered rule). Shared by the CLI and by tests
/// that need the same rule set without going through a subprocess.
pub fn build_registry(requested: &[String]) -> RuleRegistry {
    let all = requested.iter().any(|r| r == ALL_RULES_SENTINEL);
    let mut registry = RuleRegistry::new();
    for rule in RULES {
        if all || requested.iter().any(|r| r == rule.id) {
            registry.register_boxed((rule.build)());
        }
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// `build_registry` matches by string equality against `RuleDescriptor::id`,
    /// so a copy-pasted duplicate id would silently double-register a rule
    /// (double-run `check`, duplicate/overlapping `Update`s) instead of
    /// failing to compile. Assert uniqueness explicitly, mirroring
    /// `forge::tests::test_forge_ids_are_unique`.
    #[test]
    fn test_rule_ids_are_unique() {
        let ids: HashSet<&str> = RULES.iter().map(|r| r.id).collect();
        assert_eq!(
            ids.len(),
            RULES.len(),
            "duplicate RuleDescriptor::id in RULES"
        );
    }

    #[test]
    fn test_build_registry_all_enables_every_rule() {
        let registry = build_registry(&[ALL_RULES_SENTINEL.to_string()]);
        assert_eq!(registry.len(), RULES.len());
    }

    #[test]
    fn test_build_registry_unknown_id_is_a_no_op() {
        let registry = build_registry(&["not-a-real-rule".to_string()]);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_build_registry_selects_only_requested_rules() {
        let registry = build_registry(&["flake".to_string()]);
        assert_eq!(registry.len(), 1);
    }
}
