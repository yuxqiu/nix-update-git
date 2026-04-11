use crate::parser::NixNode;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Update {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub range: (usize, usize),
}

impl Update {
    pub fn new(
        field: impl Into<String>,
        old_value: impl Into<String>,
        new_value: impl Into<String>,
        range: (usize, usize),
    ) -> Self {
        Self {
            field: field.into(),
            old_value: old_value.into(),
            new_value: new_value.into(),
            range,
        }
    }
}

pub trait UpdateRule: Send + Sync {
    fn name(&self) -> &str;
    fn matches(&self, node: &NixNode) -> bool;
    fn check(&self, node: &NixNode) -> Result<Option<Vec<Update>>>;
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

    pub fn check_all(&self, node: &NixNode) -> Result<Vec<(String, Vec<Update>)>> {
        let mut results = Vec::new();
        for rule in &self.rules {
            if rule.matches(node) {
                if let Some(updates) = rule.check(node)? {
                    if !updates.is_empty() {
                        results.push((rule.name().to_string(), updates));
                    }
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
