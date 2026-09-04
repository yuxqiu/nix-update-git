use crate::parser::NixNode;
use crate::rules::traits::{CheckResult, UpdateRule};

use super::{extract, resolve};

pub struct DerivationRule {
    rule_name: String,
    func_names: Vec<String>,
}

impl DerivationRule {
    #[must_use]
    pub fn new(rule_name: &str, func_names: Vec<String>) -> Self {
        Self {
            rule_name: rule_name.to_string(),
            func_names,
        }
    }
}

impl UpdateRule for DerivationRule {
    fn name(&self) -> &str {
        &self.rule_name
    }

    fn matches(&self, node: &NixNode) -> bool {
        node.kind() == rnix::SyntaxKind::NODE_APPLY
    }

    fn check(&self, node: &NixNode) -> CheckResult {
        let Some(call) = extract::try_extract_call(&self.func_names, node) else {
            return CheckResult::empty();
        };

        let target = call.fetcher_kind().display_target(call.fetcher_parsed());

        let mut result = resolve::check_derivation_call(&self.rule_name, &call);

        for group in &mut result.groups {
            group.target.clone_from(&target);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::fetcher::is_commit_hash;

    #[test]
    fn test_is_commit_hash_valid() {
        assert!(is_commit_hash("4f56fd184ef6020626492a6f954a486d54f8b7ba"));
        assert!(is_commit_hash("0000000000000000000000000000000000000000"));
    }

    #[test]
    fn test_is_commit_hash_invalid() {
        assert!(!is_commit_hash("v1.0.0"));
        assert!(!is_commit_hash("main"));
        assert!(!is_commit_hash("short"));
        assert!(!is_commit_hash("4f56fd184ef6020626492a6f954a486d54f8b7ba0"));
        assert!(!is_commit_hash("4f56fd184ef6020626492a6f954a486d54f8b7b"));
    }
}
