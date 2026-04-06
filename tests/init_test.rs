use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

/// Creates a local git repo with an anvil.toml and source files,
/// suitable as a clone source for init tests.
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
fn test_init_clones_applies_and_writes_config() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone_dest = dir.path().join("dotfiles");
    let config_path = dir.path().join("config").join("anvil.toml");
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

    create_source_repo(&source, &manifest, &[(".zshrc", "# zshrc content")]);

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    let profiles =
        anvil::cli::init::run_init(source.to_str().unwrap(), &clone_dest, &[], &git, &ctx).unwrap();

    // Verify clone happened
    assert!(clone_dest.join("anvil.toml").exists());
    assert!(clone_dest.join(".zshrc").exists());

    // Verify symlinks were created
    assert!(dest_zshrc.exists());
    assert!(dest_zshrc.symlink_metadata().unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&dest_zshrc).unwrap(), "# zshrc content");

    // Verify correct profiles resolved
    assert_eq!(profiles, vec!["base"]);

    // Verify local config can be written with the returned values
    anvil::config::local::LocalConfig::save_to(
        &config_path,
        clone_dest.to_str().unwrap(),
        &profiles,
    )
    .unwrap();
    let config = anvil::config::local::LocalConfig::load_from(&config_path)
        .unwrap()
        .unwrap();
    assert_eq!(config.clone_dir, clone_dest.to_str().unwrap());
    assert_eq!(config.profiles, vec!["base"]);
}

#[test]
fn test_init_without_url_in_yes_mode_errors() {
    // In --yes mode, text prompt with no default returns PromptCancelled
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let result = ctx.text("Repository URL:", None);
    assert!(result.is_err());
}

#[test]
fn test_init_into_existing_directory_errors() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone_dest = dir.path().join("dotfiles");

    create_source_repo(&source, "[anvil]\nversion = \"1\"\n", &[]);

    // Pre-create the clone destination
    fs::create_dir_all(&clone_dest).unwrap();

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    let err = anvil::cli::init::run_init(source.to_str().unwrap(), &clone_dest, &[], &git, &ctx)
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("already exists"),
        "expected 'already exists' in: {msg}"
    );
}

#[test]
fn test_init_missing_manifest_errors() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone_dest = dir.path().join("dotfiles");

    // Create repo WITHOUT anvil.toml
    fs::create_dir_all(&source).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(&source)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&source)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&source)
        .output()
        .unwrap();
    fs::write(source.join("README.md"), "# my dots").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&source)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&source)
        .output()
        .unwrap();

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    let err = anvil::cli::init::run_init(source.to_str().unwrap(), &clone_dest, &[], &git, &ctx)
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("anvil.toml"),
        "expected mention of anvil.toml in: {msg}"
    );
}

#[test]
fn test_init_with_explicit_profile() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone_dest = dir.path().join("dotfiles");
    let dest_work = dir.path().join("home").join(".work");

    let manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = []

[profiles.work]
links = [
  {{ src = ".work", dest = "{}" }},
]
"#,
        dest_work.display()
    );

    create_source_repo(&source, &manifest, &[(".work", "# work config")]);

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    let profiles = anvil::cli::init::run_init(
        source.to_str().unwrap(),
        &clone_dest,
        &["work".to_string()],
        &git,
        &ctx,
    )
    .unwrap();

    assert_eq!(profiles, vec!["work"]);
    assert!(dest_work.exists());
    assert!(dest_work.symlink_metadata().unwrap().is_symlink());
}

#[test]
fn test_init_invalid_clone_url_errors() {
    let dir = tempdir().unwrap();
    let clone_dest = dir.path().join("dotfiles");

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    let err = anvil::cli::init::run_init("/nonexistent/path/to/repo", &clone_dest, &[], &git, &ctx)
        .unwrap_err();

    assert!(
        matches!(err, anvil::error::AnvilError::GitCloneFailed(_)),
        "expected GitCloneFailed, got: {err}"
    );

    // Verify partial clone was cleaned up
    assert!(!clone_dest.exists());
}

#[test]
fn test_init_nonexistent_profile_errors() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source-repo");
    let clone_dest = dir.path().join("dotfiles");

    let manifest = r#"
[anvil]
version = "1"

[profiles.base]
links = []
"#;

    create_source_repo(&source, manifest, &[]);

    let ctx = anvil::ui::UiContext::new(true, true, false);
    let git = anvil::git::ShellGit;

    let err = anvil::cli::init::run_init(
        source.to_str().unwrap(),
        &clone_dest,
        &["nonexistent".to_string()],
        &git,
        &ctx,
    )
    .unwrap_err();

    assert!(
        matches!(err, anvil::error::AnvilError::ProfileNotFound(_)),
        "expected ProfileNotFound, got: {err}"
    );
}
