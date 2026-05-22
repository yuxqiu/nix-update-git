use std::fmt;

use crate::parser::{NixNode, TextRange};

#[derive(Debug, Clone)]
pub struct Update {
    pub field: String,
    pub replacement: String,
    pub range: TextRange,
    pub rule_name: String,
    pub target: Option<String>,
}

impl Update {
    pub fn new(field: impl Into<String>, replacement: impl Into<String>, range: TextRange) -> Self {
        Self {
            field: field.into(),
            replacement: replacement.into(),
            range,
            rule_name: String::new(),
            target: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
}

/// A group of updates that must be applied atomically.
/// If any update in the group depends on a computation that fails
/// (e.g., hash prefetch in flaky network), the entire group is discarded
/// to avoid leaving the file in an inconsistent state.
#[derive(Debug, Clone)]
pub struct UpdateGroup {
    pub updates: Vec<Update>,
}

impl UpdateGroup {
    pub fn new(updates: Vec<Update>) -> Self {
        Self { updates }
    }

    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }
}

/// Warnings produced during rule checking.
///
/// These represent recoverable failures (network flakiness, missing refs,
/// invalid directives) that should be reported to the user but do not
/// abort the check. Every I/O failure inside a rule is caught and
/// converted into a warning — the rule simply skips that particular
/// update and continues.
#[derive(Debug)]
pub enum CheckWarning {
    /// Failed to prefetch/compute a hash for a fetcher call.
    HashPrefetchFailed {
        url: String,
        rev: String,
        source: anyhow::Error,
    },

    /// A `# follow:` directive could not be resolved due to an I/O error.
    FollowResolutionFailed {
        git_url: String,
        source: anyhow::Error,
    },

    /// A `# follow:branch` found no matching branch.
    FollowBranchNotFound { git_url: String, branch: String },

    /// A `# follow:regex` found no matching tags.
    FollowRegexNoMatch { git_url: String, pattern: String },

    /// A `# follow:semver` found no matching tags.
    FollowSemverNoMatch {
        git_url: String,
        requirement: String,
    },

    /// A `# follow:` directive has invalid syntax (e.g., bad semver requirement).
    InvalidFollowDirective {
        directive: String,
        source: anyhow::Error,
    },
}

impl fmt::Display for CheckWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckWarning::HashPrefetchFailed { url, rev, source } => {
                write!(
                    f,
                    "could not prefetch hash for {} @ {}: {:#}",
                    url, rev, source
                )
            }
            CheckWarning::FollowResolutionFailed { git_url, source } => {
                write!(f, "could not resolve follow for {}: {:#}", git_url, source)
            }
            CheckWarning::FollowBranchNotFound { git_url, branch } => {
                write!(f, "could not find branch '{}' for {}", branch, git_url)
            }
            CheckWarning::FollowRegexNoMatch { git_url, pattern } => {
                write!(f, "no tags matching regex '{}' for {}", pattern, git_url)
            }
            CheckWarning::FollowSemverNoMatch {
                git_url,
                requirement,
            } => {
                write!(
                    f,
                    "no tags matching semver '{}' for {}",
                    requirement, git_url
                )
            }
            CheckWarning::InvalidFollowDirective { directive, source } => {
                write!(f, "invalid follow directive '{}': {:#}", directive, source)
            }
        }
    }
}

/// Result of checking a single node against a rule.
pub struct CheckResult {
    pub groups: Vec<UpdateGroup>,
    pub warnings: Vec<CheckWarning>,
}

impl CheckResult {
    pub fn empty() -> Self {
        Self {
            groups: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_group(group: UpdateGroup) -> Self {
        Self {
            groups: vec![group],
            warnings: Vec::new(),
        }
    }

    pub fn with_warnings(warnings: Vec<CheckWarning>) -> Self {
        Self {
            groups: Vec::new(),
            warnings,
        }
    }

    pub fn merge(&mut self, other: CheckResult) {
        self.groups.extend(other.groups);
        self.warnings.extend(other.warnings);
    }
}

pub trait UpdateRule: Send + Sync {
    fn name(&self) -> &str;
    fn matches(&self, node: &NixNode) -> bool;
    fn check(&self, node: &NixNode) -> CheckResult;
}

pub struct RuleRegistry {
    rules: Vec<Box<dyn UpdateRule>>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register<R: UpdateRule + 'static>(&mut self, rule: R) {
        self.rules.push(Box::new(rule));
    }

    pub fn check_all(
        &self,
        root: &NixNode,
    ) -> (Vec<(String, Vec<UpdateGroup>)>, Vec<CheckWarning>) {
        let mut results = Vec::new();
        let mut all_warnings = Vec::new();
        for node in root.traverse() {
            for rule in &self.rules {
                if rule.matches(&node) {
                    let CheckResult { groups, warnings } = rule.check(&node);
                    all_warnings.extend(warnings);
                    if !groups.is_empty() {
                        let rule_name = rule.name().to_string();
                        let mut groups = groups;
                        for group in &mut groups {
                            for update in &mut group.updates {
                                update.rule_name = rule_name.clone();
                            }
                        }
                        results.push((rule_name, groups));
                    }
                }
            }
        }
        (results, all_warnings)
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
