use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use nix_update_git::parser::NixFile;
use nix_update_git::rules::{RuleRegistry, Update};

pub struct FileResult {
    pub file_path: PathBuf,
    pub content: String,
    pub updates_per_rule: Vec<(String, Vec<Update>)>,
}

impl FileResult {
    pub fn all_updates(&self) -> Vec<Update> {
        self.updates_per_rule
            .iter()
            .flat_map(|(_, updates)| updates)
            .cloned()
            .collect()
    }
}

pub fn check_file(file_path: &Path, registry: &RuleRegistry) -> Result<FileResult> {
    let content = fs::read_to_string(file_path)?;
    let nix_file = NixFile::parse(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", file_path.display(), e))?;
    let updates_per_rule = registry
        .check_all(&nix_file.root_node())
        .map_err(|e| anyhow::anyhow!("Error checking {}: {}", file_path.display(), e))?;
    Ok(FileResult {
        file_path: file_path.to_path_buf(),
        content,
        updates_per_rule,
    })
}
