//! Pure-Rust reimplementation of `nix-prefetch-git`.
//!
//! Clones a git repository, makes the checkout deterministic, and
//! computes the NAR SHA-256 hash. This replaces the shell-based
//! `nix-prefetch-git` from nixpkgs, removing the dependency on
//! `nix-prefetch-git`, `nix-hash`, and `nix-store`.
//!
//! # Example
//!
//! ```no_run
//! use nix_prefetch_git::{PrefetchArgs, prefetch};
//!
//! let args = PrefetchArgs {
//!     url: "https://github.com/owner/repo".to_string(),
//!     rev: "abc123def456".to_string(),
//!     fetch_submodules: false,
//!     deep_clone: false,
//!     leave_dot_git: false,
//!     fetch_lfs: false,
//!     branch_name: None,
//!     root_dir: None,
//!     sparse_checkout: vec![],
//! };
//!
//! let result = prefetch(&args).unwrap();
//! println!("SRI hash: {}", result.sri_hash);
//! println!("Nix-base32 hash: {}", result.sha256_nix);
//! println!("Resolved rev: {}", result.rev);
//! ```

pub mod nar;

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

pub use nar::NarHash;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during git prefetching.
#[derive(Debug)]
pub enum Error {
    /// The `git` command was not found on `$PATH` or `git --version`
    /// returned a non-zero exit code.
    GitUnavailable(std::io::Error),
    /// A git command exited with a non-zero status.
    GitFailed {
        /// The git subcommand that failed (e.g. `"fetch origin"`).
        command: String,
        /// The working directory in which the command was run.
        directory: String,
        /// The standard error output from git.
        stderr: String,
    },
    /// An I/O error occurred.
    Io(std::io::Error),
    /// NAR hash computation failed.
    NarHash {
        /// The path that was being hashed.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// Failed to create a temporary directory.
    TempDir(std::io::Error),
    /// Failed to resolve FETCH_HEAD after a shallow fetch.
    FetchHead {
        /// The working directory.
        directory: String,
        /// The standard error output from git.
        stderr: String,
    },
    /// Failed to set up git sparse-checkout.
    SparseCheckout(String),
    /// The checkout directory does not exist after cloning.
    CheckoutDir(String),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::GitUnavailable(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::NarHash { source, .. } => Some(source),
            Error::TempDir(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::GitUnavailable(e) => write!(f, "git is not available: {e}"),
            Error::GitFailed {
                command,
                directory,
                stderr,
            } => write!(f, "git {command} failed in {directory}: {stderr}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::NarHash { path, source } => {
                write!(f, "failed to compute NAR hash for {path}: {source}")
            }
            Error::TempDir(e) => write!(f, "failed to create temporary directory: {e}"),
            Error::FetchHead { directory, stderr } => {
                write!(f, "failed to resolve FETCH_HEAD in {directory}: {stderr}")
            }
            Error::SparseCheckout(msg) => write!(f, "sparse checkout failed: {msg}"),
            Error::CheckoutDir(path) => {
                write!(f, "checkout directory does not exist: {path}")
            }
        }
    }
}

/// A convenience alias for `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Arguments for prefetching a git repository.
///
/// Mirrors the options accepted by `nix-prefetch-git`.
#[derive(Debug, Clone)]
pub struct PrefetchArgs {
    /// The URL of the git repository to fetch.
    pub url: String,
    /// The revision (SHA-1/SHA-256 commit hash, branch, or tag) to check out.
    pub rev: String,
    /// Whether to recursively fetch submodules.
    pub fetch_submodules: bool,
    /// Whether to create a deep clone (full history). Implies `--tags`
    /// when `leave_dot_git` is also true.
    pub deep_clone: bool,
    /// Whether to leave the `.git` directories in the checkout. When false
    /// (the default), `.git` is stripped and the checkout is fully
    /// deterministic. When true, `.git` is kept but made deterministic
    /// (repacked, garbage-collected, and stripped of non-reproducible
    /// metadata).
    pub leave_dot_git: bool,
    /// Whether to fetch Git Large File Storage (LFS) objects.
    pub fetch_lfs: bool,
    /// Branch name used for the local checkout. Defaults to `"fetchgit"`.
    pub branch_name: Option<String>,
    /// An optional subdirectory of the repository to use as the top-level
    /// output (instead of the whole repository).
    pub root_dir: Option<String>,
    /// Paths for git sparse-checkout. When non-empty, only these paths
    /// are checked out.
    pub sparse_checkout: Vec<String>,
}

/// Result of prefetching a git repository.
#[derive(Debug, Clone)]
pub struct PrefetchResult {
    /// SHA-256 hash in nix-base32 format (e.g. `"0nmyp5yrzl9dbq85wy..."`).
    pub sha256_nix: String,
    /// SHA-256 hash in SRI format (e.g. `"sha256-RTt0byv..."`).
    pub sri_hash: String,
    /// The full commit hash that was checked out.
    pub rev: String,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// Prefetch a git repository and compute its NAR hash.
///
/// This is a pure-Rust replacement for `nix-prefetch-git`. It:
/// 1. Clones the repository into a temporary directory
/// 2. Makes the checkout deterministic
/// 3. Computes the NAR SHA-256 hash of the result
///
/// # Errors
///
/// Returns an error if:
/// - The `git` command is not available or fails
/// - The repository cannot be cloned or the revision cannot be checked out
/// - The NAR hash computation fails
pub fn prefetch(args: &PrefetchArgs) -> Result<PrefetchResult> {
    ensure_git_available()?;

    let branch = args.branch_name.as_deref().unwrap_or("fetchgit");

    // Create two temp directories: one for the checkout, one for
    // $HOME so git doesn't read user-level config.
    let home_dir = TempDir::new().map_err(Error::TempDir)?;
    let checkout_parent = TempDir::new().map_err(Error::TempDir)?;
    let clone_dir = checkout_parent.path().join("clone");
    std::fs::create_dir_all(&clone_dir).map_err(Error::Io)?;

    // --- Initialise repo ---------------------------------------------------
    git(&clone_dir, &home_dir, &["init", "--initial-branch=master"])?;
    git(
        &clone_dir,
        &home_dir,
        &["remote", "add", "origin", &args.url],
    )?;

    // Disable GPG signing so commits never hang.
    git(
        &clone_dir,
        &home_dir,
        &["config", "commit.gpgsign", "false"],
    )?;
    git(&clone_dir, &home_dir, &["config", "tag.gpgsign", "false"])?;

    // --- Sparse checkout ---------------------------------------------------
    if !args.sparse_checkout.is_empty() {
        git(
            &clone_dir,
            &home_dir,
            &["config", "remote.origin.partialclonefilter", "blob:none"],
        )?;
        let stdin_data = args.sparse_checkout.join("\n");
        let mut child = Command::new("git")
            .current_dir(&clone_dir)
            .args(["sparse-checkout", "set", "--stdin"])
            .env("HOME", home_dir.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMS", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(Error::Io)?;
        {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Failed to open stdin for git sparse-checkout set",
                ))
            })?;
            stdin.write_all(stdin_data.as_bytes()).map_err(Error::Io)?;
        }
        let status = child.wait().map_err(Error::Io)?;
        if !status.success() {
            return Err(Error::SparseCheckout(
                "git sparse-checkout set failed".to_string(),
            ));
        }
    }

    // --- Fetch -------------------------------------------------------------
    // Track whether we did a shallow fetch. After a shallow fetch with
    // --no-tags, tag/branch names may not be resolvable locally (they're
    // only available via FETCH_HEAD), so we need to use FETCH_HEAD for
    // checkout instead of the original rev string.
    let shallow_fetch = if args.deep_clone {
        // Full clone with optional tags.
        let mut fetch_args = vec!["fetch".to_string()];
        if args.leave_dot_git {
            fetch_args.push("--tags".to_string());
        } else {
            fetch_args.push("--no-tags".to_string());
        }
        fetch_args.push("origin".to_string());
        git(
            &clone_dir,
            &home_dir,
            &fetch_args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        )?;
        false
    } else {
        // Try shallow fetch first; fall back to deep fetch on failure
        // (e.g. dumb HTTP protocol doesn't support shallow clones).
        let shallow_ok = git_quiet(
            &clone_dir,
            &home_dir,
            &["fetch", "--no-tags", "--depth=1", "origin", &args.rev],
        );
        if shallow_ok {
            true
        } else {
            // Shallow fetch failed — deep fetch.
            git(&clone_dir, &home_dir, &["fetch", "--no-tags", "origin"])?;
            false
        }
    };

    // --- Determine object type & check out ---------------------------------
    // Set a user identity so git commit-tree works without prompting.
    git(
        &clone_dir,
        &home_dir,
        &["config", "user.email", "nix-prefetch-git@localhost"],
    )?;
    git(
        &clone_dir,
        &home_dir,
        &["config", "user.name", "nix-prefetch-git"],
    )?;

    // After a shallow fetch, the rev (e.g. a tag name like "v0.4.1") may
    // not be resolvable locally because --no-tags prevents local tag ref
    // creation. Use FETCH_HEAD instead, which always points to the fetched
    // commit. After a deep fetch or deep clone, all remote refs are
    // available locally so the original rev string works.
    let checkout_target = if shallow_fetch {
        // Resolve FETCH_HEAD to a full commit hash.
        git_output(&clone_dir, &home_dir, &["rev-parse", "FETCH_HEAD"])
            .map_err(|e| match e {
                Error::GitFailed {
                    directory, stderr, ..
                } => Error::FetchHead { directory, stderr },
                other => other,
            })?
            .trim()
            .to_string()
    } else {
        args.rev.clone()
    };

    // Determine what kind of object the checkout target is.
    let obj_type = git_output(&clone_dir, &home_dir, &["cat-file", "-t", &checkout_target])
        .unwrap_or_default()
        .trim()
        .to_string();

    match obj_type.as_str() {
        "commit" | "tag" => {
            git(
                &clone_dir,
                &home_dir,
                &["checkout", "-b", branch, &checkout_target],
            )?;
        }
        "tree" => {
            let msg = format!("Commit created from tree hash {}", checkout_target);
            let commit_id = git_output(
                &clone_dir,
                &home_dir,
                &["commit-tree", &checkout_target, "-m", &msg],
            )?;
            git(
                &clone_dir,
                &home_dir,
                &["checkout", "-b", branch, commit_id.trim()],
            )?;
        }
        _ => {
            // Could be an abbreviated hash or a ref that doesn't exist
            // locally yet. Try checkout directly.
            git(
                &clone_dir,
                &home_dir,
                &["checkout", "-b", branch, &checkout_target],
            )?;
        }
    }

    // --- Resolve full revision ---------------------------------------------
    // Always resolve via the checked-out branch, which is guaranteed to
    // exist regardless of how we fetched.
    let full_rev = git_output(
        &clone_dir,
        &home_dir,
        &["rev-parse", &format!("refs/heads/{}", branch)],
    )?
    .trim()
    .to_string();

    // --- Submodules --------------------------------------------------------
    if args.fetch_submodules {
        if args.deep_clone || args.leave_dot_git {
            git(
                &clone_dir,
                &home_dir,
                &[
                    "submodule",
                    "update",
                    "--init",
                    "--recursive",
                    "--checkout",
                    "--progress",
                ],
            )?;
        } else {
            git(
                &clone_dir,
                &home_dir,
                &[
                    "submodule",
                    "update",
                    "--init",
                    "--recursive",
                    "--checkout",
                    "--progress",
                    "--depth=1",
                ],
            )?;
        }
    }

    // --- LFS ---------------------------------------------------------------
    if args.fetch_lfs {
        git(&clone_dir, &home_dir, &["lfs", "install"])?;
        git(&clone_dir, &home_dir, &["lfs", "pull"])?;
    }

    // --- Deterministic cleanup / .git removal ------------------------------
    if args.leave_dot_git {
        make_deterministic_repos(&clone_dir, &home_dir)?;
    } else {
        remove_dot_git_dirs(&clone_dir)?;
    }

    // --- Compute NAR hash --------------------------------------------------
    let hash_dir = match &args.root_dir {
        Some(subdir) => clone_dir.join(subdir),
        None => clone_dir.to_path_buf(),
    };

    // The checkout directory may have been moved/deleted by cleanup,
    // but we're still inside TempDir so it should exist.
    if !hash_dir.exists() {
        return Err(Error::CheckoutDir(hash_dir.display().to_string()));
    }

    let nar_hash = nar::hash_path(&hash_dir)?;

    Ok(PrefetchResult {
        sha256_nix: nar_hash.nix32,
        sri_hash: nar_hash.sri,
        rev: full_rev,
    })
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

/// Check that `git` is available on `$PATH`.
fn ensure_git_available() -> Result<()> {
    let status = Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(Error::GitUnavailable)?;
    if !status.success() {
        return Err(Error::GitUnavailable(std::io::Error::other(
            "git --version returned non-zero",
        )));
    }
    Ok(())
}

/// Run a git command in `dir` with `HOME` set to `home_dir`.
/// Returns an error if the command fails.
fn git(dir: &Path, home_dir: &TempDir, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("HOME", home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMS", "0")
        .output()
        .map_err(Error::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::GitFailed {
            command: args.join(" "),
            directory: dir.display().to_string(),
            stderr: stderr.trim().to_string(),
        });
    }
    Ok(())
}

/// Like [`git`], but returns `false` on failure instead of bailing.
fn git_quiet(dir: &Path, home_dir: &TempDir, args: &[&str]) -> bool {
    Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("HOME", home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMS", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .is_some_and(|s| s.success())
}

/// Run a git command and capture its stdout.
fn git_output(dir: &Path, home_dir: &TempDir, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("HOME", home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMS", "0")
        .output()
        .map_err(Error::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::GitFailed {
            command: args.join(" "),
            directory: dir.display().to_string(),
            stderr: stderr.trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// Deterministic repo helpers
// ---------------------------------------------------------------------------

/// Make the `.git` directory of a repo (and all submodule repos)
/// deterministic by stripping non-reproducible metadata and
/// repacking/gc-ing.
fn make_deterministic_repos(dir: &Path, home_dir: &TempDir) -> Result<()> {
    // Find all .git entries (directories or gitdir files) under `dir`.
    let git_entries = find_dot_git_entries(dir)?;
    for git_entry in &git_entries {
        // The checkout directory is the directory containing the .git entry.
        // This is where git commands should run (matches upstream's
        // `dirname "$gitdir"`).
        let checkout_dir = git_entry.parent().unwrap_or(dir).to_path_buf();

        // Determine the actual .git directory where logs/, hooks/, etc.
        // reside — this is NOT the same as the checkout directory.
        let git_dir = if git_entry.is_dir() {
            // .git is a regular directory — use it directly.
            git_entry.clone()
        } else {
            // .git is a file containing "gitdir: <path>" — resolve the
            // real git directory from the gitdir content.
            let content = std::fs::read_to_string(git_entry).map_err(Error::Io)?;
            let gitdir = content.trim().strip_prefix("gitdir: ").unwrap_or("");
            // gitdir paths are relative to the checkout directory (the
            // directory containing the .git file), NOT the clone root.
            checkout_dir.join(gitdir)
        };

        make_deterministic_repo(&checkout_dir, &git_dir, home_dir)?;
    }
    Ok(())
}

/// Make a single git repository deterministic.
///
/// Removes non-reproducible files, remote branches, unreachable tags,
/// and then repacks + gc's with deterministic settings.
fn make_deterministic_repo(checkout_dir: &Path, git_dir: &Path, home_dir: &TempDir) -> Result<()> {
    // Remove non-deterministic files and directories.
    let remove_paths: Vec<PathBuf> = [
        "logs",
        "hooks",
        "index",
        "FETCH_HEAD",
        "ORIG_HEAD",
        "config",
    ]
    .iter()
    .map(|p| git_dir.join(p))
    .collect();

    for path in &remove_paths {
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(Error::Io)?;
            } else {
                std::fs::remove_file(path).map_err(Error::Io)?;
            }
        }
    }

    // Remove refs/remotes/origin/HEAD if it exists.
    let origin_head = git_dir
        .join("refs")
        .join("remotes")
        .join("origin")
        .join("HEAD");
    if origin_head.exists() {
        std::fs::remove_file(&origin_head).map_err(Error::Io)?;
    }

    // Remove all remote branches.
    let branch_output = Command::new("git")
        .current_dir(checkout_dir)
        .args(["branch", "-r"])
        .env("HOME", home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output();
    if let Ok(output) = branch_output
        && output.status.success()
    {
        for branch in String::from_utf8_lossy(&output.stdout).lines() {
            let branch = branch.trim();
            if !branch.is_empty() {
                let _ = Command::new("git")
                    .current_dir(checkout_dir)
                    .args(["branch", "-rD", branch])
                    .env("HOME", home_dir.path())
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .output();
            }
        }
    }

    // Remove tags not reachable from HEAD. Keep the tag HEAD is on.
    let maybe_head_tag = Command::new("git")
        .current_dir(checkout_dir)
        .args(["tag", "--points-at", "HEAD"])
        .env("HOME", home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output();
    let head_tag = maybe_head_tag
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let tags_output = Command::new("git")
        .current_dir(checkout_dir)
        .args(["tag", "--contains", "HEAD"])
        .env("HOME", home_dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output();
    if let Ok(output) = tags_output
        && output.status.success()
    {
        for tag in String::from_utf8_lossy(&output.stdout).lines() {
            let tag = tag.trim();
            if !tag.is_empty() && tag != head_tag {
                let _ = Command::new("git")
                    .current_dir(checkout_dir)
                    .args(["tag", "-d", tag])
                    .env("HOME", home_dir.path())
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .output();
            }
        }
    }

    // Single-threaded repack for determinism.
    git(checkout_dir, home_dir, &["config", "pack.threads", "1"])?;
    git(checkout_dir, home_dir, &["repack", "-A", "-d", "-f"])?;

    // Remove config again (repack may have recreated it).
    let config_path = git_dir.join("config");
    if config_path.exists() {
        std::fs::remove_file(&config_path).map_err(Error::Io)?;
    }

    // Garbage collect.
    git(
        checkout_dir,
        home_dir,
        &["gc", "--prune=all", "--keep-largest-pack"],
    )?;

    Ok(())
}

/// Recursively find all `.git` entries (directories or files) under `dir`.
fn find_dot_git_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    visit_dirs(dir, &mut entries)?;
    Ok(entries)
}

/// Recursive directory visitor that collects paths named `.git`.
fn visit_dirs(dir: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == ".git") {
                result.push(path);
            } else if path.is_dir() {
                visit_dirs(&path, result)?;
            }
        }
    }
    Ok(())
}

/// Remove all `.git` entries (directories or files) under `dir`.
fn remove_dot_git_dirs(dir: &Path) -> Result<()> {
    let git_entries = find_dot_git_entries(dir)?;
    for entry in &git_entries {
        if entry.is_dir() {
            std::fs::remove_dir_all(entry).map_err(Error::Io)?;
        } else {
            std::fs::remove_file(entry).map_err(Error::Io)?;
        }
    }
    Ok(())
}
