//! Revertibility and installer-choice pins (temp dirs only; never real $HOME).

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::tempdir;

fn anvil_bin() -> assert_cmd::Command {
    cargo_bin_cmd!("anvil")
}

struct Harness {
    _root: tempfile::TempDir,
    repo: PathBuf,
    home: PathBuf,
    xdg_config: PathBuf,
    xdg_state: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let root = tempdir().unwrap();
        let repo = root.path().join("dots");
        let home = root.path().join("home");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&home).unwrap();
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
        Self {
            _root: root,
            repo,
            home,
            xdg_config,
            xdg_state,
        }
    }

    fn write_manifest(&self, dest: &Path, extra_link_fields: &str) {
        let extra = if extra_link_fields.is_empty() {
            String::new()
        } else {
            format!(", {extra_link_fields}")
        };
        let manifest = format!(
            r#"
[anvil]
version = "1"
default_profile = "base"
[profiles.base]
links = [{{ src = "file", dest = "{}"{extra} }}]
"#,
            dest.display()
        );
        fs::write(self.repo.join("anvil.toml"), manifest).unwrap();
    }

    fn cmd(&self) -> assert_cmd::Command {
        let mut c = anvil_bin();
        c.env("XDG_CONFIG_HOME", &self.xdg_config)
            .env("XDG_STATE_HOME", &self.xdg_state)
            .env("HOME", &self.home);
        c
    }
}

#[test]
fn force_overwrite_then_undo_restores_original_contents() {
    let h = Harness::new();
    fs::write(h.repo.join("file"), "from-repo\n").unwrap();
    let dest = h.home.join("file");
    fs::write(&dest, "local-original\n").unwrap();
    h.write_manifest(&dest, "");

    h.cmd().args(["apply", "-y", "--force"]).assert().success();
    assert!(dest.symlink_metadata().unwrap().file_type().is_symlink());

    h.cmd().args(["undo", "-y"]).assert().success();
    assert_eq!(fs::read_to_string(&dest).unwrap(), "local-original\n");
}

#[test]
fn apply_force_replaces_dangling_symlink_when_src_exists() {
    let h = Harness::new();
    fs::write(h.repo.join("file"), "from-repo\n").unwrap();
    let dest = h.home.join("file");
    symlink(h.home.join("does-not-exist"), &dest).unwrap();
    h.write_manifest(&dest, "");

    h.cmd().args(["apply", "-y", "--force"]).assert().success();
    assert!(dest.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read_link(&dest).unwrap(), h.repo.join("file"));
}

#[test]
fn add_refuses_path_already_symlinked_into_repo() {
    let h = Harness::new();
    fs::write(
        h.repo.join("anvil.toml"),
        r#"
[anvil]
version = "1"
default_profile = "base"
[profiles.base]
links = []
"#,
    )
    .unwrap();
    let src = h.repo.join(".zshrc");
    fs::write(&src, "export FOO=1\n").unwrap();
    let dest = h.home.join(".zshrc");
    symlink(&src, &dest).unwrap();

    h.cmd()
        .args(["add", dest.to_str().unwrap(), "-p", "base", "-y"])
        .assert()
        .failure();
    assert!(src.exists(), "repo copy must stay intact");
    assert_eq!(fs::read_to_string(&src).unwrap(), "export FOO=1\n");
    assert!(
        dest.symlink_metadata().unwrap().file_type().is_symlink(),
        "dest must remain the original symlink"
    );
}

#[test]
fn apply_accepts_aur_helper_flag() {
    let h = Harness::new();
    fs::write(h.repo.join("file"), "x\n").unwrap();
    let dest = h.home.join("file");
    h.write_manifest(&dest, "");

    h.cmd()
        .args([
            "apply",
            "--dry-run",
            "-y",
            "--packages",
            "--aur-helper",
            "anzen",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("anzen"));
}

#[test]
fn undo_list_prints_journals() {
    let h = Harness::new();
    fs::write(h.repo.join("file"), "from-repo\n").unwrap();
    let dest = h.home.join("file");
    fs::write(&dest, "local\n").unwrap();
    h.write_manifest(&dest, "");

    h.cmd().args(["apply", "-y", "--force"]).assert().success();

    h.cmd()
        .args(["undo", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("entries"));
}
