use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

/// Creates a local git repo with an anvil.toml and source files,
/// suitable as a clone source for sync tests.
fn create_source_repo(dir: &Path, manifest: &str, files: &[(&str, &str)]) {
    fs::create_dir_all(dir).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();

    fs::write(dir.join("anvil.toml"), manifest).unwrap();
    for (name, content) in files {
        if let Some(parent) = Path::new(name).parent() {
            fs::create_dir_all(dir.join(parent)).unwrap();
        }
        fs::write(dir.join(name), content).unwrap();
    }

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir)
        .output()
        .unwrap();
}

#[test]
fn test_sync_pulls_and_reapplies() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone = dir.path().join("dotfiles");
    let dest_zshrc = dir.path().join("home").join(".zshrc");

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
        dest_zshrc.display()
    );

    create_source_repo(&source, &manifest, &[(".zshrc", "# zshrc v1")]);

    // Clone the source (simulating what init would have done)
    Command::new("git")
        .args(["clone"])
        .arg(&source)
        .arg(&clone)
        .output()
        .unwrap();

    // First apply via sync — links the initial files
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;
    anvil::cli::sync::run_sync(&clone, &git, &ctx).unwrap();

    assert!(dest_zshrc.exists());
    assert!(dest_zshrc.symlink_metadata().unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&dest_zshrc).unwrap(), "# zshrc v1");

    // Now update the source with a new file and updated manifest
    let dest_vimrc = dir.path().join("home").join(".vimrc");
    let updated_manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = [
  {{ src = ".zshrc", dest = "{}" }},
  {{ src = ".vimrc", dest = "{}" }},
]
"#,
        dest_zshrc.display(),
        dest_vimrc.display()
    );
    fs::write(source.join("anvil.toml"), &updated_manifest).unwrap();
    fs::write(source.join(".vimrc"), "\" vimrc content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&source)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add vimrc"])
        .current_dir(&source)
        .output()
        .unwrap();

    // Sync again — should pull the new commit and create the new link
    anvil::cli::sync::run_sync(&clone, &git, &ctx).unwrap();

    assert!(dest_vimrc.exists());
    assert!(dest_vimrc.symlink_metadata().unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&dest_vimrc).unwrap(), "\" vimrc content");

    // Original link still correct
    assert!(dest_zshrc.symlink_metadata().unwrap().is_symlink());
}

#[test]
fn test_sync_already_up_to_date_still_applies() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone = dir.path().join("dotfiles");
    let dest_file = dir.path().join("home").join(".config");

    let manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = [
  {{ src = "config", dest = "{}" }},
]
"#,
        dest_file.display()
    );

    create_source_repo(&source, &manifest, &[("config", "content")]);

    Command::new("git")
        .args(["clone"])
        .arg(&source)
        .arg(&clone)
        .output()
        .unwrap();

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    // First sync creates the link
    anvil::cli::sync::run_sync(&clone, &git, &ctx).unwrap();
    assert!(dest_file.exists());

    // Remove the symlink manually (simulating a broken link scenario)
    fs::remove_file(&dest_file).unwrap();
    assert!(!dest_file.exists());

    // Sync again with no upstream changes — should still re-apply the link
    anvil::cli::sync::run_sync(&clone, &git, &ctx).unwrap();
    assert!(dest_file.exists());
    assert!(dest_file.symlink_metadata().unwrap().is_symlink());
}

#[test]
fn test_sync_no_repo_found_errors() {
    // discover_repo_with(None) + no ~/.dotfiles should produce a helpful error
    let result = anvil::config::local::discover_repo_with(None);
    if let Err(e) = result {
        let msg = e.to_string();
        assert!(
            msg.contains("anvil init"),
            "expected guidance to run 'anvil init' in: {msg}"
        );
    }
    // If ~/.dotfiles happens to exist on the test machine, the fallback
    // succeeds — that's fine, the error path is still tested in local.rs
}
