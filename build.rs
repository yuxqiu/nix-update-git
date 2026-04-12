use std::env;
use std::process::Command;

fn main() {
    let git_hash = env::var("NIX_UPDATE_GIT_HASH")
        .ok()
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        String::from_utf8(out.stdout)
                            .ok()
                            .map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo::rustc-env=GIT_HASH={}", git_hash);
    println!("cargo::rerun-if-env-changed=NIX_UPDATE_GIT_HASH");
    println!("cargo::rerun-if-changed=.git/HEAD");
}
