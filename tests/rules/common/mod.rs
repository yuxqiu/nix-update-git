use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct TestRepo {
    pub path: PathBuf,
    _dir: TempDir,
}

fn run_git(args: &[&str], current_dir: &Path) -> std::process::Output {
    std::process::Command::new("git")
        .args(args)
        .current_dir(current_dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("failed to run git")
}

impl TestRepo {
    pub fn new(tags: &[&str]) -> Self {
        let dir = TempDir::new().expect("failed to create temp dir");
        let path = dir.path().to_path_buf();

        run_git(&["init"], &path);
        run_git(&["config", "user.email", "test@test.com"], &path);
        run_git(&["config", "user.name", "Test"], &path);
        run_git(&["config", "commit.gpgsign", "false"], &path);
        run_git(&["config", "tag.gpgsign", "false"], &path);

        fs::write(path.join("README.md"), "init").unwrap();
        run_git(&["add", "."], &path);
        run_git(&["commit", "-m", "init"], &path);

        for tag in tags {
            fs::write(path.join("README.md"), format!("commit for {}", tag)).unwrap();
            run_git(&["add", "."], &path);
            run_git(&["commit", "-m", &format!("commit for {}", tag)], &path);
            run_git(&["tag", tag], &path);
        }

        Self { path, _dir: dir }
    }

    pub fn path_str(&self) -> &str {
        self.path.to_str().expect("non-utf8 path")
    }

    #[allow(dead_code)]
    pub fn head_sha(&self) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.path)
            .output()
            .expect("failed to get HEAD sha");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[allow(dead_code)]
    pub fn add_commit(&self, content: &str) {
        fs::write(self.path.join("README.md"), content).unwrap();
        run_git(&["add", "."], &self.path);
        run_git(
            &["commit", "-m", &format!("commit: {}", content)],
            &self.path,
        );
    }
}
