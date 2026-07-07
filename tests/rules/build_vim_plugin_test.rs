use super::common::TestRepo;
use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_build_vim_plugin_detects_version_and_rev_update() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = vimUtils.buildVimPlugin rec {{
    pname = "foo";
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
        .stdout(predicates::str::contains("build-vim-plugin.version"));
}

#[test]
fn test_build_vim_plugin_no_update_when_version_is_latest() {
    let repo = TestRepo::new(&["v1.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = vimUtils.buildVimPlugin rec {{
    pname = "foo";
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
fn test_build_vim_plugin_updates_when_rev_matches_version() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = vimUtils.buildVimPlugin rec {{
    pname = "foo";
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
        .stdout(predicates::str::contains("build-vim-plugin.version"))
        .stdout(predicates::str::contains("fetchgit.rev"));
}

#[test]
fn test_build_vim_plugin_update_applies() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);

    let nix_content = format!(
        r#"{{
  foo = vimUtils.buildVimPlugin rec {{
    pname = "foo";
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
fn test_build_vim_plugin_pinned() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = vimUtils.buildVimPlugin rec {{ # pin
    pname = "foo";
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
fn test_build_vim_plugin_enabled_by_default() {
    let repo = TestRepo::new(&["v1.0.0", "v2.0.0"]);
    let sha = repo.head_sha();

    let nix_content = format!(
        r#"{{
  foo = vimUtils.buildVimPlugin rec {{
    pname = "foo";
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

    // No --rules flag: build-vim-plugin should be on by default.
    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("build-vim-plugin.version"));
}
