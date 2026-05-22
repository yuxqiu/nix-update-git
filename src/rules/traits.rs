use crate::parser::{NixNode, TextRange};
use anyhow::Result;

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

pub trait UpdateRule: Send + Sync {
    fn name(&self) -> &str;
    fn matches(&self, node: &NixNode) -> bool;
    fn check(&self, node: &NixNode) -> Result<Option<Vec<UpdateGroup>>>;
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

    pub fn check_all(&self, root: &NixNode) -> Result<Vec<(String, Vec<UpdateGroup>)>> {
        let mut results = Vec::new();
        for node in root.traverse() {
            for rule in &self.rules {
                if rule.matches(&node)
                    && let Some(mut groups) = rule.check(&node)?
                    && !groups.is_empty()
                {
                    let rule_name = rule.name().to_string();
                    for group in &mut groups {
                        for update in &mut group.updates {
                            update.rule_name = rule_name.clone();
                        }
                    }
                    results.push((rule_name, groups));
                }
            }
        }
        Ok(results)
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
