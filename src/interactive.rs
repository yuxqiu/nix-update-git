use std::io::{self, Write};

use nix_update_git::presentation::{FileDiff, Hunk, new_line, old_line};
use nix_update_git::rules::Update;

/// Walks every hunk across all files in order, prompting `[y,n,a,q,?]` for
/// each — the same vocabulary as `git add -p`. Returns one `Vec<Update>` per
/// input `FileDiff`, in the same order, containing only the accepted
/// updates for that file. `- `/`+ ` lines are colored red/green when `color`
/// is `true`.
pub fn select(diffs: &[FileDiff], color: bool) -> Vec<Vec<Update>> {
    let mut result: Vec<Vec<Update>> = diffs.iter().map(|_| Vec::new()).collect();

    let hunks: Vec<(usize, &Hunk)> = diffs
        .iter()
        .enumerate()
        .flat_map(|(i, d)| d.hunks.iter().map(move |h| (i, h)))
        .collect();
    let total = hunks.len();
    let mut accept_all = false;

    for (n, (file_idx, hunk)) in hunks.into_iter().enumerate() {
        if accept_all {
            result[file_idx].extend(hunk.updates.clone());
            continue;
        }

        let target = hunk
            .target
            .as_deref()
            .map(|t| format!(": {t}"))
            .unwrap_or_default();
        println!(
            "Hunk {}/{} — {}{}",
            n + 1,
            total,
            diffs[file_idx].path.display(),
            target
        );
        for change in &hunk.changes {
            println!("    {}", change.field);
            println!("    {}", old_line(&format!("- {}", change.old), color));
            println!("    {}", new_line(&format!("+ {}", change.new), color));
        }

        match prompt() {
            Answer::Yes => result[file_idx].extend(hunk.updates.clone()),
            Answer::No => {}
            Answer::All => {
                accept_all = true;
                result[file_idx].extend(hunk.updates.clone());
            }
            Answer::Quit => break,
        }
    }

    result
}

enum Answer {
    Yes,
    No,
    All,
    Quit,
}

fn parse_answer(input: &str) -> Option<Answer> {
    match input.trim().to_lowercase().as_str() {
        "y" => Some(Answer::Yes),
        "n" | "" => Some(Answer::No),
        "a" => Some(Answer::All),
        "q" => Some(Answer::Quit),
        _ => None,
    }
}

fn prompt() -> Answer {
    loop {
        print!("Apply this hunk? [y,n,a,q,?] ");
        io::stdout().flush().ok();
        let mut input = String::new();
        // `read_line` returning 0 bytes means stdin hit EOF (e.g. a
        // non-interactive invocation): quit rather than looping forever
        // re-interpreting an empty read as "skip this hunk".
        let bytes_read = io::stdin().read_line(&mut input).unwrap_or(0);
        if bytes_read == 0 {
            return Answer::Quit;
        }
        if let Some(answer) = parse_answer(&input) {
            return answer;
        }
        println!("y - apply this hunk");
        println!("n - skip this hunk");
        println!("a - apply this hunk and all remaining hunks");
        println!("q - quit; apply nothing further");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_answer() {
        assert!(matches!(parse_answer("y"), Some(Answer::Yes)));
        assert!(matches!(parse_answer("Y"), Some(Answer::Yes)));
        assert!(matches!(parse_answer("n"), Some(Answer::No)));
        assert!(matches!(parse_answer(""), Some(Answer::No)));
        assert!(matches!(parse_answer("\n"), Some(Answer::No)));
        assert!(matches!(parse_answer("a"), Some(Answer::All)));
        assert!(matches!(parse_answer("q"), Some(Answer::Quit)));
        assert!(parse_answer("?").is_none());
        assert!(parse_answer("x").is_none());
    }
}
