use super::common::TestRepo;
use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn nix_prefetch_git_is_available() -> bool {
    Command::new("nix-prefetch-git")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

#[test]
fn test_mk_derivation_detects_version_and_rev_update() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "{}";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("mkDerivation.version"));
}

#[test]
fn test_mk_derivation_no_update_when_version_is_latest() {
    let repo = TestRepo::new(&["v1.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "0000000000000000000000000000000000000000";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
fn test_mk_derivation_updates_when_rev_matches_version() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v1.0.0";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
        .stdout(predicates::str::contains("mkDerivation.version"))
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_mk_derivation_update_applies() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "0000000000000000000000000000000000000000";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
fn test_mk_derivation_pinned() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{ # pin
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "{}";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
fn test_mk_derivation_with_pkgs_prefix() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = pkgs.stdenv.mkDerivation rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "{}";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("mkDerivation.version"));
}

#[test]
fn test_mk_derivation_no_version_attr_skips() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    name = "foo-1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "{}";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
fn test_mk_derivation_version_not_version_string_skips() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    name = "foo-${{version}}";
    version = "main";
    src = fetchgit {{
      url = "{}";
      rev = "{}";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
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
fn test_mk_derivation_hash_update() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "0000000000000000000000000000000000000000";
      hash = "";
    }};
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
        .stdout(predicates::str::contains("mkDerivation.version"))
        .stdout(predicates::str::contains("fetchgit.rev"))
        .stdout(predicates::str::contains("fetchgit.hash"));
}

#[test]
fn test_mk_derivation_interpolated_rev_updates_version_and_hash_only() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v${{version}}";
      hash = "";
    }};
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "stdout: {}",
        stdout
    );
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
}

#[test]
fn test_mk_derivation_populates_empty_rev_and_hash_from_version() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "";
      hash = "";
    }};
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    assert!(
        !stdout.contains("mkDerivation.version"),
        "stdout: {}",
        stdout
    );
}

#[test]
fn test_mk_derivation_empty_rev_rehashes_existing_hash() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "";
      hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    }};
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    assert!(
        !stdout.contains("mkDerivation.version"),
        "stdout: {}",
        stdout
    );
}

#[test]
fn test_mk_derivation_empty_hash_with_matching_rev_updates_hash_only() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v1.0.0";
      hash = "";
    }};
  }};
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
    assert!(
        !stdout.contains("mkDerivation.version"),
        "stdout: {}",
        stdout
    );
}

#[test]
fn test_mk_derivation_updates_tag_and_version_when_tag_matches_version() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      tag = "v1.0.0";
    }};
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
        .stdout(predicates::str::contains("mkDerivation.version"))
        .stdout(predicates::str::contains("fetchgit.tag"));
}

#[test]
fn test_mk_derivation_updates_builtins_ref_and_version_when_ref_matches_version() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation rec {{
    version = "v1.0.0";
    src = builtins.fetchGit {{
      url = "{}";
      ref = "v1.0.0";
    }};
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
        .stdout(predicates::str::contains("mkDerivation.version"))
        .stdout(predicates::str::contains("builtins.fetchGit.ref"));
}

#[test]
fn test_mk_derivation_lambda_wrapped_detects_update() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v1.0.0";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
  }});
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
        .stdout(predicates::str::contains("mkDerivation.version"))
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_mk_derivation_lambda_wrapped_empty_rev_and_hash() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "";
      hash = "";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
}

#[test]
fn test_mk_derivation_lambda_wrapped_interpolated_rev() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: {{
    version = "1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v${{finalAttrs.version}}";
      hash = "";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "stdout: {}",
        stdout
    );
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    // rev is interpolated — should NOT produce a rev update
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
}

#[test]
fn test_fetcher_excluded_from_lambda_wrapped_mk_derivation() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: {{
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v1.0.0";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The fetcher inside the lambda-wrapped mkDerivation should be
    // handled by the mk-derivation rule, not the fetcher rule.
    assert!(
        !stdout.contains("fetcher"),
        "Fetcher rule should not process src inside lambda-wrapped mkDerivation, got: {}",
        stdout
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "mk-derivation rule should detect version update, got: {}",
        stdout
    );
}

#[test]
fn test_mk_derivation_rec_lambda_dual_interpolation_bare_version() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    // Both rec and finalAttrs are present; ${version} is valid
    // because rec allows bare self-references.
    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: rec {{
    version = "1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v${{version}}";
      hash = "";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "should detect version update, stdout: {}",
        stdout
    );
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    // rev is interpolated — should NOT produce a rev update
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
}

#[test]
fn test_mk_derivation_rec_lambda_dual_interpolation_final_attrs_version() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    // Both rec and finalAttrs are present; ${finalAttrs.version} is valid
    // because the lambda parameter provides the self-reference.
    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: rec {{
    version = "1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v${{finalAttrs.version}}";
      hash = "";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "should detect version update, stdout: {}",
        stdout
    );
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    // rev is interpolated — should NOT produce a rev update
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
}

#[test]
fn test_mk_derivation_rec_and_final_attrs_together() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "{}";
      sha256 = "0nmyp5yrzl9dbq85wyiimsj9fklb8637a1936nw7zzvlnzkgh28n";
    }};
  }});
}}"#,
        repo.path_str(),
        sha
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("mkDerivation.version"));
}

#[test]
fn test_mk_derivation_rec_and_final_attrs_interpolated_version() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v${{version}}";
      hash = "";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "should detect version update, stdout: {}",
        stdout
    );
    // rev is interpolated — should NOT produce a rev update, only version + hash
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
}

#[test]
fn test_mk_derivation_rec_and_final_attrs_interpolated_final_attrs_version() {
    if !nix_prefetch_git_is_available() {
        return;
    }

    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = stdenv.mkDerivation (finalAttrs: rec {{
    name = "foo-${{version}}";
    version = "v1.0.0";
    src = fetchgit {{
      url = "{}";
      rev = "v${{finalAttrs.version}}";
      hash = "";
    }};
  }});
}}"#,
        repo.path_str()
    );

    let nix_dir = tempdir().unwrap();
    let nix_path = nix_dir.path().join("test.nix");
    fs::write(&nix_path, &nix_content).unwrap();

    let out = Command::cargo_bin("nix-update-git")
        .unwrap()
        .arg(nix_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("mkDerivation.version"),
        "should detect version update, stdout: {}",
        stdout
    );
    // rev is interpolated with finalAttrs.version — should NOT produce a rev update
    assert!(stdout.contains("fetchgit.hash"), "stdout: {}", stdout);
    assert!(!stdout.contains("fetchgit.rev"), "stdout: {}", stdout);
}
