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
        .stdout(predicates::str::contains("mkDerivation.version"))
        .stdout(predicates::str::contains("fetchgit.rev"));
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
fn test_mk_derivation_skips_when_rev_is_version() {
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

    // When rev is a version (not a hash), the mkDerivation rule skips it.
    // The fetcher rule handles version-in-rev updates instead, so we expect
    // the fetcher rule to produce an update — not "No updates found".
    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("fetcher"))
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
