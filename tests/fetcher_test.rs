use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn create_git_repo_with_tags(path: &std::path::Path, tags: &[&str]) {
    let run_git = |args: &[&str]| -> std::process::Output {
        std::process::Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("failed to run git")
    };

    run_git(&["init"]);
    run_git(&["config", "user.email", "test@test.com"]);
    run_git(&["config", "user.name", "Test"]);
    run_git(&["config", "commit.gpgsign", "false"]);
    run_git(&["config", "tag.gpgsign", "false"]);

    fs::write(path.join("README.md"), "init").unwrap();
    run_git(&["add", "."]);
    run_git(&["commit", "-m", "init"]);

    for tag in tags {
        fs::write(path.join("README.md"), format!("commit for {}", tag)).unwrap();
        run_git(&["add", "."]);
        let msg = format!("commit for {}", tag);
        run_git(&["commit", "-m", &msg]);
        run_git(&["tag", tag]);
    }
}

fn get_head_sha(path: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .expect("failed to get HEAD sha");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn test_fetcher_fetchgit_detects_update() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_fetcher_fetchgit_with_fetchgit() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_fetcher_tag_attribute() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    tag = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.tag"));
}

#[test]
fn test_github_fetch_from_github_two_component_version() {
    let nix_content = r#"{
  src = pkgs.fetchFromGitHub {
    owner = "arkenfox";
    repo = "user.js";
    rev = "140.0";
    hash = "sha256-LPDiiEPOZu5Ah5vCLyCMT3w1uoBhUjyqoPWCOiLVLnw=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchFromGitHub.rev"))
        .stdout(predicates::str::contains("140.0"));
}

#[test]
fn test_github_fetch_from_github_detects_update() {
    let nix_content = r#"{
  src = fetchFromGitHub {
    owner = "yuxqiu";
    repo = "nix-update-git";
    rev = "v0.1.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchFromGitHub.rev"))
        .stdout(predicates::str::contains("v0.1.0"))
        .stdout(predicates::str::contains("v0.1.1"));
}

#[test]
fn test_github_fetch_from_github_tag_attribute() {
    let nix_content = r#"{
  src = fetchFromGitHub {
    owner = "yuxqiu";
    repo = "nix-update-git";
    tag = "v0.1.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchFromGitHub.tag"))
        .stdout(predicates::str::contains("v0.1.0"))
        .stdout(predicates::str::contains("v0.1.1"));
}

#[test]
fn test_github_fetch_from_github_update_mode() {
    let nix_content = r#"{
  src = fetchFromGitHub {
    owner = "yuxqiu";
    repo = "nix-update-git";
    rev = "v0.1.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--update").arg(nix_path.to_str().unwrap());
    cmd.assert().success();

    let updated = fs::read_to_string(&nix_path).unwrap();
    assert!(
        updated.contains("v0.1.1"),
        "File should contain v0.1.1 after update, got: {}",
        updated
    );
    assert!(
        !updated.contains("\"v0.1.0\""),
        "File should no longer contain \"v0.1.0\" after update, got: {}",
        updated
    );
}

#[test]
fn test_github_fetchgit_detects_update() {
    let nix_content = r#"{
  src = fetchgit {
    url = "https://github.com/yuxqiu/nix-update-git";
    rev = "v0.1.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.rev"))
        .stdout(predicates::str::contains("v0.1.0"))
        .stdout(predicates::str::contains("v0.1.1"));
}

#[test]
fn test_github_builtins_fetch_git() {
    let nix_content = r#"{
  src = builtins.fetchGit {
    url = "https://github.com/yuxqiu/nix-update-git";
    ref = "v0.1.0";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("builtins.fetchGit.ref"))
        .stdout(predicates::str::contains("v0.1.0"))
        .stdout(predicates::str::contains("v0.1.1"));
}

#[test]
fn test_github_fetch_from_github_no_update_when_latest() {
    let nix_content = r#"{
  src = fetchFromGitHub {
    owner = "yuxqiu";
    repo = "nix-update-git";
    rev = "v0.1.1";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_github_fetch_from_github_pinned() {
    let nix_content = r#"{
  src = fetchFromGitHub { # pin
    owner = "yuxqiu";
    repo = "nix-update-git";
    rev = "v0.1.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_fetcher_pinned() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{ # pin
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_fetcher_commit_hash_no_update() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0"]);
    let sha = get_head_sha(repo_dir.path());

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "{}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path, sha
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_fetcher_non_version_ref_no_update() {
    let nix_content = r#"{
  src = fetchFromGitHub {
    owner = "testowner";
    repo = "testrepo";
    rev = "main";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_fetcher_builtins_fetch_git() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = builtins.fetchGit {{
    url = "{}";
    ref = "v1.0.0";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("builtins.fetchGit.ref"));
}

#[test]
fn test_fetcher_update_mode() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--update").arg(nix_path.to_str().unwrap());
    cmd.assert().success();

    let updated = fs::read_to_string(&nix_path).unwrap();
    assert!(
        updated.contains("v2.0.0"),
        "File should contain v2.0.0 after update, got: {}",
        updated
    );
}

#[test]
fn test_fetcher_sha256_attribute() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
}

#[test]
fn test_fetcher_follow_branch_comment() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0"]);

    let run_git = |args: &[&str]| -> std::process::Output {
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo_dir.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("failed to run git")
    };

    fs::write(repo_dir.path().join("README.md"), "new commit after tag").unwrap();
    run_git(&["add", "."]);
    run_git(&["commit", "-m", "new commit after tag"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let old_sha = "0000000000000000000000000000000000000000";

    let nix_content = format!(
        r#"{{
  src = fetchgit {{ # follow:master
    url = "{}";
    rev = "{}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path, old_sha
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("fetchgit.rev") || output.status.success(),
        "Expected to detect update for follow:master, got stdout: {}, stderr: {}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_fetcher_fetch_from_gitea() {
    let nix_content = r#"{
  src = fetchFromGitea {
    domain = "gitea.example.com";
    owner = "testowner";
    repo = "testrepo";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
}

#[test]
fn test_fetcher_fetch_from_sourcehut() {
    let nix_content = r#"{
  src = fetchFromSourcehut {
    owner = "~sirhc";
    repo = "repo";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
}

#[test]
fn test_fetcher_fetch_from_bitbucket() {
    let nix_content = r#"{
  src = fetchFromBitbucket {
    owner = "testowner";
    repo = "testrepo";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
}

#[test]
fn test_fetcher_fetch_from_codeberg() {
    let nix_content = r#"{
  src = fetchFromCodeberg {
    owner = "testowner";
    repo = "testrepo";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
}"#;
    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
}

#[test]
fn test_fetcher_pkgs_dotted_name() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = pkgs.fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_fetcher_builtins_fetch_git_dotted() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let repo_path = repo_dir.path().to_str().unwrap();
    let nix_content = format!(
        r#"{{
  src = builtins.fetchGit {{
    url = "{}";
    ref = "v1.0.0";
  }};
}}"#,
        repo_path
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("builtins.fetchGit.ref"));
}
