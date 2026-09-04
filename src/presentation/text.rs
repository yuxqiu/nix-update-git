use super::diff::FileDiff;

/// Renders a file's diff as human-readable text, or `None` if there's
/// nothing to show. Rule names are only included when `verbose` is set.
pub fn render(diff: &FileDiff, verbose: bool) -> Option<String> {
    if diff.hunks.is_empty() {
        return None;
    }

    let mut out = diff.path.display().to_string();
    out.push('\n');

    for hunk in &diff.hunks {
        let header = match (verbose, hunk.target.as_deref()) {
            (true, Some(target)) => Some(format!("  [{}] {}", hunk.rule_name, target)),
            (true, None) => Some(format!("  [{}]", hunk.rule_name)),
            (false, Some(target)) => Some(format!("  {}", target)),
            (false, None) => None,
        };
        if let Some(header) = header {
            out.push_str(&header);
            out.push('\n');
        }
        for change in &hunk.changes {
            out.push_str(&format!("    {}\n", change.field));
            out.push_str(&format!("    - {}\n", change.old));
            out.push_str(&format!("    + {}\n", change.new));
        }
    }

    Some(out.trim_end().to_string())
}
