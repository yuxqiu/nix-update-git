use crate::parser::{NixNode, TextRange};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Update {
    pub field: String,
    pub replacement: String,
    pub range: TextRange,
    pub rule_name: String,
    pub detail: Option<String>,
}

impl Update {
    pub fn new(field: impl Into<String>, replacement: impl Into<String>, range: TextRange) -> Self {
        Self {
            field: field.into(),
            replacement: replacement.into(),
            range,
            rule_name: String::new(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
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

    pub fn check_all(&self, root: &NixNode) -> Result<Vec<(String, Vec<Update>)>> {
        let mut results = Vec::new();
        for node in root.traverse() {
            for rule in &self.rules {
                if rule.matches(&node)
                    && let Some(mut updates) = rule.check(&node)?
                    && !updates.is_empty()
                {
                    let rule_name = rule.name().to_string();
                    for update in &mut updates {
                        update.rule_name = rule_name.clone();
                    }
                    results.push((rule_name, updates));
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
