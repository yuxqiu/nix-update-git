//! Minimal ANSI coloring for diff lines: red for removed, green for added —
//! the same convention as `git diff`. This module has no I/O and no
//! environment-detection logic of its own; callers decide whether to apply
//! color (based on `--color`/TTY/`NO_COLOR` detection in `main.rs`).

use anstyle::{AnsiColor, Style};

const OLD: Style = AnsiColor::Red.on_default();
const NEW: Style = AnsiColor::Green.on_default();

fn paint(style: Style, text: &str) -> String {
    format!("{style}{text}{style:#}")
}

/// Styles a removed-line fragment (e.g. `"- \"v1.0.0\""`) red, or returns it
/// unchanged when `color` is `false`.
#[must_use]
pub fn old_line(text: &str, color: bool) -> String {
    if color {
        paint(OLD, text)
    } else {
        text.to_string()
    }
}

/// Styles an added-line fragment (e.g. `"+ \"v2.0.0\""`) green, or returns it
/// unchanged when `color` is `false`.
#[must_use]
pub fn new_line(text: &str, color: bool) -> String {
    if color {
        paint(NEW, text)
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_old_line_disabled_is_unchanged() {
        assert_eq!(old_line("- \"v1.0.0\"", false), "- \"v1.0.0\"");
    }

    #[test]
    fn test_new_line_disabled_is_unchanged() {
        assert_eq!(new_line("+ \"v2.0.0\"", false), "+ \"v2.0.0\"");
    }

    #[test]
    fn test_old_line_enabled_wraps_in_ansi_red() {
        let styled = old_line("- \"v1.0.0\"", true);
        assert!(styled.starts_with("\x1b["));
        assert!(styled.ends_with("\x1b[0m"));
        assert!(styled.contains("- \"v1.0.0\""));
    }

    #[test]
    fn test_new_line_enabled_wraps_in_ansi_green() {
        let styled = new_line("+ \"v2.0.0\"", true);
        assert!(styled.starts_with("\x1b["));
        assert!(styled.ends_with("\x1b[0m"));
        assert!(styled.contains("+ \"v2.0.0\""));
    }
}
