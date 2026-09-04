use std::io::{self, Write};

use nix_update_git::presentation::{FileDiff, Hunk};
use nix_update_git::rules::Update;

/// Walks every hunk across all files in order, prompting `[y,n,a,q,?]` for
/// each — the same vocabulary as `git add -p`. Returns one `Vec<Update>` per
/// input `FileDiff`, in the same order, containing only the accepted
/// updates for that file.
pub fn select(diffs: &[FileDiff]) -> Vec<Vec<Update>> {
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
            println!("    - {}", change.old);
            println!("    + {}", change.new);
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

fn prompt() -> Answer {
    loop {
        print!("Apply this hunk? [y,n,a,q,?] ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        match input.trim().to_lowercase().as_str() {
            "y" => return Answer::Yes,
            "n" | "" => return Answer::No,
            "a" => return Answer::All,
            "q" => return Answer::Quit,
            _ => {
                println!("y - apply this hunk");
                println!("n - skip this hunk");
                println!("a - apply this hunk and all remaining hunks");
                println!("q - quit; apply nothing further");
            }
        }
    }
}
