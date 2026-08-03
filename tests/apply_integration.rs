//! Integration: plan-based apply against a temp repo (no real home).

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::tempdir;

fn anvil_bin() -> assert_cmd::Command {
    cargo_bin_cmd!("anvil")
}

#[test]
fn apply_dry_run_with_local_config() {
    let root = tempdir().unwrap();
    let repo = root.path().join("dots");
    let home = root.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(repo.join(".zshrc"), "export FOO=1\n").unwrap();

    let dest = home.join(".zshrc");
    let manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = [
  {{ src = ".zshrc", dest = "{}" }},
]
"#,
        dest.display()
    );
    fs::write(repo.join("anvil.toml"), manifest).unwrap();

    let xdg_config = root.path().join("xdg-config");
    fs::create_dir_all(xdg_config.join("anvil")).unwrap();
    fs::write(
        xdg_config.join("anvil/config.toml"),
        format!(
            "repo_path = \"{}\"\nprofiles = [\"base\"]\n",
            repo.display()
        ),
    )
    .unwrap();

    let xdg_state = root.path().join("xdg-state");
    fs::create_dir_all(&xdg_state).unwrap();

    anvil_bin()
        .env("XDG_CONFIG_HOME", &xdg_config)
        .env("XDG_STATE_HOME", &xdg_state)
        .env("HOME", &home)
        .args(["apply", "--dry-run", "-y"])
        .assert()
        .success();

    assert!(!dest.exists());
}

#[test]
fn apply_creates_symlink() {
    let root = tempdir().unwrap();
    let repo = root.path().join("dots");
    let home = root.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(repo.join(".zshrc"), "export FOO=1\n").unwrap();

    let dest = home.join(".zshrc");
    let manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = [
  {{ src = ".zshrc", dest = "{}" }},
]
"#,
        dest.display()
    );
    fs::write(repo.join("anvil.toml"), manifest).unwrap();

    let xdg_config = root.path().join("xdg-config");
    fs::create_dir_all(xdg_config.join("anvil")).unwrap();
    fs::write(
        xdg_config.join("anvil/config.toml"),
        format!(
            "repo_path = \"{}\"\nprofiles = [\"base\"]\n",
            repo.display()
        ),
    )
    .unwrap();
    let xdg_state = root.path().join("xdg-state");
    fs::create_dir_all(&xdg_state).unwrap();

    anvil_bin()
        .env("XDG_CONFIG_HOME", &xdg_config)
        .env("XDG_STATE_HOME", &xdg_state)
        .env("HOME", &home)
        .args(["apply", "-y"])
        .assert()
        .success();

    assert!(dest.symlink_metadata().unwrap().file_type().is_symlink());
    let target = fs::read_link(&dest).unwrap();
    assert_eq!(target, repo.join(".zshrc"));
}

#[test]
fn help_lists_commands() {
    anvil_bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("apply"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("undo"));
}

#[test]
fn force_overwrite_backs_up() {
    let root = tempdir().unwrap();
    let repo = root.path().join("dots");
    let home = root.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(repo.join("file"), "from-repo\n").unwrap();
    let dest = home.join("file");
    fs::write(&dest, "local\n").unwrap();

    let manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"
[profiles.base]
links = [{{ src = "file", dest = "{}" }}]
"#,
        dest.display()
    );
    fs::write(repo.join("anvil.toml"), manifest).unwrap();

    let xdg_config = root.path().join("xdg-config");
    fs::create_dir_all(xdg_config.join("anvil")).unwrap();
    fs::write(
        xdg_config.join("anvil/config.toml"),
        format!(
            "repo_path = \"{}\"\nprofiles = [\"base\"]\n",
            repo.display()
        ),
    )
    .unwrap();
    let xdg_state = root.path().join("xdg-state");
    fs::create_dir_all(&xdg_state).unwrap();

    anvil_bin()
        .env("XDG_CONFIG_HOME", &xdg_config)
        .env("XDG_STATE_HOME", &xdg_state)
        .env("HOME", &home)
        .args(["apply", "-y", "--force"])
        .assert()
        .success();

    assert!(dest.symlink_metadata().unwrap().file_type().is_symlink());
    let backups = xdg_state.join("anvil/backups");
    assert!(backups.exists());
}
