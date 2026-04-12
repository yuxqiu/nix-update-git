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

#[test]
fn test_flake_input_no_ref() {
    let temp_dir = tempdir().unwrap();
    let flake_content = r#"{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs";
    };
  };

  outputs = { self, nixpkgs }: {};
}"#;
    let flake_path = temp_dir.path().join("flake.nix");
    fs::write(&flake_path, flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_flake_input_with_non_version_ref() {
    let temp_dir = tempdir().unwrap();
    let flake_content = r#"{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs";
      ref = "main";
    };
  };

  outputs = { self, nixpkgs }: {};
}"#;
    let flake_path = temp_dir.path().join("flake.nix");
    fs::write(&flake_path, flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_non_flake_file() {
    let temp_dir = tempdir().unwrap();
    let nix_content = r#"let pkgs = import <nixpkgs> {}; in pkgs.hello"#;
    let nix_path = temp_dir.path().join("hello.nix");
    fs::write(&nix_path, nix_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(nix_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_flake_input_detects_version_update() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs = {{
    mylib = {{
      url = "git+file://{}";
      ref = "v1.0.0";
    }};
  }};

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("inputs.mylib.ref"))
        .stdout(predicates::str::contains("v1.0.0"))
        .stdout(predicates::str::contains("v2.0.0"));
}

#[test]
fn test_flake_input_pinned() {
    let temp_dir = tempdir().unwrap();
    let flake_content = r#"{
  inputs = {
    nixpkgs = { # pin
      url = "github:NixOS/nixpkgs";
      ref = "v1.0.0";
    };
  };

  outputs = { self, nixpkgs }: {};
}"#;
    let flake_path = temp_dir.path().join("flake.nix");
    fs::write(&flake_path, flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_flake_input_pinned_on_ref() {
    let temp_dir = tempdir().unwrap();
    let flake_content = r#"{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs";
      ref = "v1.0.0"; # pin
    };
  };

  outputs = { self, nixpkgs }: {};
}"#;
    let flake_path = temp_dir.path().join("flake.nix");
    fs::write(&flake_path, flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_flake_input_dotted_form() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs.mylib.url = "git+file://{}";
  inputs.mylib.ref = "v1.0.0";

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("inputs.mylib.ref"));
}

#[test]
fn test_flake_input_inline_ref_github() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v0.6.0", "v0.7.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs = {{
    mylib = {{
      url = "git+file://{}?ref=v0.6.0";
    }};
  }};

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("inputs.mylib.url"))
        .stdout(predicates::str::contains("v0.6.0"))
        .stdout(predicates::str::contains("v0.7.0"));
}

#[test]
fn test_flake_input_inline_ref_bare_string() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v0.6.0", "v0.7.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs = {{
    mylib = "git+file://{}?ref=v0.6.0";
  }};

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("inputs.mylib.url"))
        .stdout(predicates::str::contains("v0.6.0"))
        .stdout(predicates::str::contains("v0.7.0"));
}

#[test]
fn test_flake_input_inline_ref_no_update() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v0.6.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs = {{
    mylib = {{
      url = "git+file://{}?ref=v0.6.0";
    }};
  }};

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_flake_input_inline_ref_pinned() {
    let temp_dir = tempdir().unwrap();
    let flake_content = r#"{
  inputs = {
    nix-flatpak = { # pin
      url = "github:gmodena/nix-flatpak/?ref=v0.6.0";
    };
  };

  outputs = { self, nix-flatpak }: {};
}"#;
    let flake_path = temp_dir.path().join("flake.nix");
    fs::write(&flake_path, flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--verbose").arg(flake_path.to_str().unwrap());
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("No updates found"));
}

#[test]
fn test_update_mode_ref() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v1.0.0", "v2.0.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs = {{
    mylib = {{
      url = "git+file://{}";
      ref = "v1.0.0";
    }};
  }};

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--update").arg(flake_path.to_str().unwrap());
    cmd.assert().success();

    let updated = fs::read_to_string(&flake_path).unwrap();
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
fn test_update_mode_inline_ref() {
    let repo_dir = tempdir().unwrap();
    create_git_repo_with_tags(repo_dir.path(), &["v0.6.0", "v0.7.0"]);

    let flake_dir = tempdir().unwrap();
    let repo_path = repo_dir.path().to_str().unwrap();
    let flake_content = format!(
        r#"{{
  inputs = {{
    mylib = {{
      url = "git+file://{}?ref=v0.6.0";
    }};
  }};

  outputs = {{ self, mylib }}: {{}};
}}"#,
        repo_path
    );

    let flake_path = flake_dir.path().join("flake.nix");
    fs::write(&flake_path, &flake_content).unwrap();

    let mut cmd = Command::cargo_bin("nix-update-git").unwrap();
    cmd.arg("--update").arg(flake_path.to_str().unwrap());
    cmd.assert().success();

    let updated = fs::read_to_string(&flake_path).unwrap();
    assert!(
        updated.contains("v0.7.0"),
        "File should contain v0.7.0 after update, got: {}",
        updated
    );
}
