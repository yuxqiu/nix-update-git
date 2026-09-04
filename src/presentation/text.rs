use std::fmt::Write as _;

use super::color::{new_line, old_line};
use super::diff::FileDiff;

/// Renders a file's diff as human-readable text, or `None` if there's
/// nothing to show. Rule names are only included when `verbose` is set.
/// `- `/`+ ` lines are colored red/green when `color` is `true`.
#[must_use]
pub fn render(diff: &FileDiff, verbose: bool, color: bool) -> Option<String> {
    if diff.hunks.is_empty() {
        return None;
    }

    let mut out = diff.path.display().to_string();
    out.push('\n');

    for (i, hunk) in diff.hunks.iter().enumerate() {
        if i > 0 {
            // Separate hunks visually — without this, two hunks with no
            // header (or identical field names) run together with no way
            // to tell where one ends and the next begins.
            out.push('\n');
        }
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
            let _ = writeln!(out, "    {}", change.field);
            let _ = writeln!(out, "    {}", old_line(&format!("- {}", change.old), color));
            let _ = writeln!(out, "    {}", new_line(&format!("+ {}", change.new), color));
        }
    }

    Some(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::render;
    use crate::presentation::diff::{FileDiff, Hunk, LineChange};

    fn hunk(rule_name: &str, target: Option<&str>, field: &str, old: &str, new: &str) -> Hunk {
        Hunk {
            rule_name: rule_name.to_string(),
            target: target.map(str::to_string),
            changes: vec![LineChange {
                field: field.to_string(),
                old: old.to_string(),
                new: new.to_string(),
            }],
            updates: Vec::new(),
        }
    }

    #[test]
    fn test_render_no_hunks_returns_none() {
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![],
        };
        assert_eq!(render(&diff, false, false), None);
    }

    #[test]
    fn test_render_single_hunk_with_target_non_verbose() {
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![hunk(
                "flake",
                Some("github.com/foo/mylib"),
                "inputs.foo.ref",
                "\"v1.0.0\"",
                "\"v2.0.0\"",
            )],
        };
        assert_eq!(
            render(&diff, false, false).unwrap(),
            "flake.nix\n  github.com/foo/mylib\n    inputs.foo.ref\n    - \"v1.0.0\"\n    + \"v2.0.0\""
        );
    }

    #[test]
    fn test_render_single_hunk_without_target_non_verbose_has_no_header() {
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![hunk(
                "fetcher",
                None,
                "fetchpatch.url",
                "\"old\"",
                "\"new\"",
            )],
        };
        assert_eq!(
            render(&diff, false, false).unwrap(),
            "flake.nix\n    fetchpatch.url\n    - \"old\"\n    + \"new\""
        );
    }

    #[test]
    fn test_render_verbose_shows_rule_name_even_without_target() {
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![hunk(
                "fetcher",
                None,
                "fetchpatch.url",
                "\"old\"",
                "\"new\"",
            )],
        };
        assert_eq!(
            render(&diff, true, false).unwrap(),
            "flake.nix\n  [fetcher]\n    fetchpatch.url\n    - \"old\"\n    + \"new\""
        );
    }

    #[test]
    fn test_render_multiple_hunks_are_separated_by_a_blank_line() {
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![
                hunk(
                    "flake",
                    Some("github.com/foo/mylib"),
                    "inputs.foo.ref",
                    "\"v1.0.0\"",
                    "\"v2.0.0\"",
                ),
                hunk(
                    "flake",
                    Some("gitlab.com/bar/otherlib"),
                    "inputs.bar.ref",
                    "\"v0.5.0\"",
                    "\"v1.0.0\"",
                ),
            ],
        };
        let rendered = render(&diff, false, false).unwrap();
        assert_eq!(
            rendered,
            "flake.nix\n  github.com/foo/mylib\n    inputs.foo.ref\n    - \"v1.0.0\"\n    + \"v2.0.0\"\n\n  gitlab.com/bar/otherlib\n    inputs.bar.ref\n    - \"v0.5.0\"\n    + \"v1.0.0\""
        );
        // Exactly one blank line between the two hunk blocks, none elsewhere.
        assert_eq!(rendered.matches("\n\n").count(), 1);
    }

    #[test]
    fn test_render_two_headerless_hunks_are_still_separated() {
        // Regression case for the original bug: two hunks with no target
        // (e.g. unresolved fetchpatch/fetchTarball URLs) and identical field
        // names used to run together with nothing to tell them apart.
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![
                hunk("fetcher", None, "fetchpatch.url", "\"a-old\"", "\"a-new\""),
                hunk("fetcher", None, "fetchpatch.url", "\"b-old\"", "\"b-new\""),
            ],
        };
        let rendered = render(&diff, false, false).unwrap();
        assert_eq!(rendered.matches("\n\n").count(), 1);
    }

    #[test]
    fn test_render_with_color_wraps_old_new_lines_in_ansi() {
        let diff = FileDiff {
            path: PathBuf::from("flake.nix"),
            hunks: vec![hunk(
                "flake",
                Some("github.com/foo/mylib"),
                "inputs.foo.ref",
                "\"v1.0.0\"",
                "\"v2.0.0\"",
            )],
        };
        let rendered = render(&diff, false, true).unwrap();
        assert!(rendered.contains("\x1b[31m- \"v1.0.0\"\x1b[0m"));
        assert!(rendered.contains("\x1b[32m+ \"v2.0.0\"\x1b[0m"));
        // The field line and header are left unstyled.
        assert!(rendered.contains("    inputs.foo.ref\n"));
        assert!(rendered.contains("  github.com/foo/mylib\n"));
    }
}
