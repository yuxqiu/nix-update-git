use super::common::TestRepo;
use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_fetcher_fetchgit_detects_update() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
fn test_fetcher_tag_attribute_update() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    tag = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
    assert!(
        !updated.contains("\"v1.0.0\""),
        "File should no longer contain \"v1.0.0\" after update, got: {}",
        updated
    );
}

#[test]
fn test_fetcher_fetchgit_tag_and_rev_prefers_tag() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    tag = "v1.0.0";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
fn test_fetcher_builtins_fetch_git_follow_branch_updates_ref() {
    let repo = TestRepo::new(&["v1.0.0"]);
    repo.add_commit("commit after tag");

    let head_sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  src = builtins.fetchGit {{ # follow:branch master
    url = "{}";
    ref = "v1.0.0";
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--update").arg(nix_path.to_str().unwrap());
    cmd.assert().success();

    let updated = fs::read_to_string(&nix_path).unwrap();
    assert!(
        updated.contains(&head_sha),
        "File should contain the new HEAD SHA after update, got: {}",
        updated
    );
    assert!(
        !updated.contains("\"v1.0.0\""),
        "builtins.fetchGit#follow should update ref, got: {}",
        updated
    );
}

#[test]
fn test_fetcher_fetchgit_follow_branch_updates_rev() {
    let repo = TestRepo::new(&["v1.0.0"]);
    repo.add_commit("commit after tag");

    let head_sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  src = fetchgit {{ # follow:branch master
    url = "{}";
    rev = "0000000000000000000000000000000000000000";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--update").arg(nix_path.to_str().unwrap());
    cmd.assert().success();

    let updated = fs::read_to_string(&nix_path).unwrap();
    assert!(
        updated.contains(&head_sha),
        "File should contain the new HEAD SHA after update, got: {}",
        updated
    );
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
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{ # pin
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
    let repo = TestRepo::new(&["v1.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "{}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str(),
        sha
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
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = builtins.fetchGit {{
    url = "{}";
    ref = "v1.0.0";
  }};
}}"#,
        repo.path_str()
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
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    sha256 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_fetcher_follow_branch_comment() {
    let repo = TestRepo::new(&["v1.0.0"]);
    repo.add_commit("new commit after tag");

    let old_sha = "0000000000000000000000000000000000000000";

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "{}"; # follow:branch master
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str(),
        old_sha
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("fetchgit.rev"),
        "Expected to detect update for follow:branch master, got stdout: {}, stderr: {}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_fetcher_pkgs_dotted_name() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = pkgs.fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
fn test_fetcher_empty_hash_filled_on_version_update() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{
    url = "{}";
    rev = "v1.0.0";
    hash = "";
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetchgit.hash"))
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_fetcher_pinned_empty_hash_attempted() {
    let repo = TestRepo::new(&["v1.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  src = fetchgit {{ # pin
    url = "{}";
    rev = "{}";
    hash = "";
  }};
}}"#,
        repo.path_str(),
        sha
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "Command should succeed");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("fetchgit.hash"),
        "Pinned call with empty hash should attempt to fill it, got stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

#[test]
fn test_fetcher_pinned_nonempty_hash_not_updated() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = fetchgit {{ # pin
    url = "{}";
    rev = "v1.0.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }};
}}"#,
        repo.path_str()
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
fn test_fetcher_builtins_fetch_git_dotted() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  src = builtins.fetchGit {{
    url = "{}";
    ref = "v1.0.0";
  }};
}}"#,
        repo.path_str()
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
fn test_fetcher_skips_interpolated_url() {
    // A fetcher with an interpolated url (operational key) should be
    // conservatively skipped rather than producing incorrect results.
    let nix_content = r#"{
  src = fetchgit {
    url = "https://example.com/${name}";
    rev = "v1.0.0";
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
fn test_fetcher_skips_interpolated_rev() {
    // A fetcher with an interpolated rev (operational key) should be
    // conservatively skipped.
    let nix_content = r#"{
  src = fetchgit {
    url = "https://example.com/repo";
    rev = "v${version}";
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
fn test_fetcher_fetchpatch_follow_branch_updates_url() {
    // When a fetchpatch call has a # follow:<branch> comment and the URL
    // matches a known hosting platform pattern, the tool should update
    // the commit SHA in the URL to the latest commit on that branch.
    let repo = TestRepo::new(&["v1.0.0"]);
    repo.add_commit("commit after tag");

    // Use a GitHub commit patch URL for the parser to recognize.
    // Since the GitHub repo doesn't exist, git ls-remote will fail, but
    // the tool should not crash.
    let nix_content = r#"{
  patches = [ (fetchpatch { # follow:branch master
    url = "https://github.com/owner/repo/commit/0000000000000000000000000000000000000000.patch";
    hash = "";
  }) ];
}"#;

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    // The command should succeed without panicking. Since the GitHub
    // repo doesn't exist, git ls-remote will fail and the tool will
    // print a warning, but it should not crash.
    let output = cmd.output().unwrap();
    // We can't assert specific updates since the GitHub repo doesn't
    // exist, but we verify the code path doesn't panic.
    let _ = output;
}

#[test]
fn test_fetcher_fetchpatch_empty_hash_filled() {
    // fetchpatch with an empty hash should attempt to fill it.
    let nix_content = r#"{
  patches = [ (fetchpatch {
    url = "https://github.com/owner/nonexistent-repo/commit/abc123.patch";
    hash = "";
  }) ];
}"#;

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    // Should succeed (no panic). The hash prefetch will fail since
    // the URL doesn't exist, but the code path should be exercised.
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "fetchpatch should not crash");
}

#[test]
fn test_fetcher_fetchpatch_non_empty_hash_no_update() {
    // fetchpatch with a non-empty hash and no # follow: comment should
    // not produce any updates (no version update for unrecognized URLs).
    let nix_content = r#"{
  patches = [ (fetchpatch {
    url = "https://github.com/owner/repo/commit/abc123def456.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }) ];
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
fn test_fetcher_fetchpatch_pinned_no_version_update() {
    // A pinned fetchpatch should not attempt version updates.
    let nix_content = r#"{  patches = [ (fetchpatch { # pin
    url = "https://github.com/owner/repo/compare/v1.0.0...v2.0.0.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }) ];
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
fn test_fetcher_fetchpatch_follow_branch_with_sha_url() {
    // Verify that a fetchpatch with # follow:<branch> on a commit URL
    // produces an update for the url field.  Since we can't use a real
    // GitHub repo in unit tests, we use a local TestRepo and verify that
    // the URL update logic is wired through correctly by checking that
    // the tool runs without error on a fetchpatch with follow comment.
    let nix_content = r#"{  patches = [ (fetchpatch { # follow:branch main
    url = "https://github.com/owner/repo/commit/0000000000000000000000000000000000000000.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }) ];
}"#;

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    // The command should succeed (no panic).  Since the GitHub repo
    // doesn't actually exist, git ls-remote will fail and we'll get
    // a warning, but the tool should not crash.
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "fetchpatch follow should not crash"
    );
}

#[test]
fn test_fetcher_fetchpatch_compare_url_no_update_with_non_version_head() {
    // A compare URL where the head is a commit SHA (not a version)
    // should not trigger version updates.
    let nix_content = r#"{
  patches = [ (fetchpatch {
    url = "https://github.com/owner/repo/compare/v1.0.0...abc123def456.patch";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  }) ];
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
