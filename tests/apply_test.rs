use std::fs;
use std::path::Path;

use tempfile::tempdir;

/// Creates a minimal dotfiles repo fixture with an anvil.toml manifest
/// and source files. Returns the repo directory path.
fn setup_repo(dir: &Path, manifest: &str, files: &[(&str, &str)]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("anvil.toml"), manifest).unwrap();
    for (name, content) in files {
        if let Some(parent) = Path::new(name).parent() {
            fs::create_dir_all(dir.join(parent)).unwrap();
        }
        fs::write(dir.join(name), content).unwrap();
    }
}

#[test]
fn test_apply_creates_symlinks() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");
    let dest_dir = dir.path().join("home");
    fs::create_dir_all(&dest_dir).unwrap();

    let dest_zshrc = dest_dir.join(".zshrc");
    let dest_gitconfig = dest_dir.join(".gitconfig");

    let manifest = format!(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = [
  {{ src = ".zshrc", dest = "{}" }},
  {{ src = ".gitconfig", dest = "{}" }},
]
"#,
        dest_zshrc.display(),
        dest_gitconfig.display()
    );

    setup_repo(
        &repo,
        &manifest,
        &[(".zshrc", "# zshrc"), (".gitconfig", "[user]\nname = me")],
    );

    // Use the library to apply
    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let profiles = vec!["base".to_string()];

    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();

    // Verify symlinks were created
    assert!(dest_zshrc.exists());
    assert!(dest_gitconfig.exists());
    assert!(dest_zshrc.symlink_metadata().unwrap().is_symlink());
    assert_eq!(fs::read_link(&dest_zshrc).unwrap(), repo.join(".zshrc"));
    assert_eq!(fs::read_to_string(&dest_zshrc).unwrap(), "# zshrc");
}

#[test]
fn test_apply_idempotent() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");
    let dest = dir.path().join("home").join(".zshrc");

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

    setup_repo(&repo, &manifest, &[(".zshrc", "# zshrc")]);

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let profiles = vec!["base".to_string()];

    // Apply twice
    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();
    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();

    // Still a valid symlink
    assert!(dest.symlink_metadata().unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "# zshrc");
}

#[test]
fn test_apply_with_conflict_yes_skips() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");
    let dest = dir.path().join("home").join(".zshrc");

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

    setup_repo(&repo, &manifest, &[(".zshrc", "repo version")]);

    // Create a conflicting file at dest
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    fs::write(&dest, "local version").unwrap();

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    // --yes mode should skip conflicts (non-destructive)
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let profiles = vec!["base".to_string()];

    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();

    // File should be unchanged (skipped)
    assert!(!dest.symlink_metadata().unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "local version");
}

#[test]
fn test_apply_dry_run_no_changes() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");
    let dest = dir.path().join("home").join(".zshrc");

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

    setup_repo(&repo, &manifest, &[(".zshrc", "# zshrc")]);

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let ctx = anvil::ui::UiContext::new(true, true, true); // dry_run = true
    let profiles = vec!["base".to_string()];

    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();

    // Dest should NOT exist
    assert!(!dest.exists());
}

#[test]
fn test_apply_creates_parent_dirs() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");
    let dest = dir
        .path()
        .join("home")
        .join("deep")
        .join("nested")
        .join(".config");

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
        dest.display()
    );

    setup_repo(&repo, &manifest, &[("config", "contents")]);

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let profiles = vec!["base".to_string()];

    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();

    assert!(dest.exists());
    assert!(dest.symlink_metadata().unwrap().is_symlink());
}

#[test]
fn test_apply_empty_profile_succeeds() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");

    let manifest = r#"
[anvil]
version = "1"
default_profile = "empty"

[profiles.empty]
links = []
"#;

    setup_repo(&repo, manifest, &[]);

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let ctx = anvil::ui::UiContext::new(true, true, false);
    let profiles = vec!["empty".to_string()];

    // Should succeed without errors
    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();
}

#[test]
fn test_apply_nonexistent_profile_errors() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");

    let manifest = r#"
[anvil]
version = "1"

[profiles.base]
links = []
"#;

    setup_repo(&repo, manifest, &[]);

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let result =
        anvil::cli::apply::resolve_profiles(&["nonexistent".to_string()], &manifest_parsed);
    assert!(result.is_err());
}

#[test]
fn test_resolve_profiles_uses_default() {
    let manifest = anvil::config::Manifest::parse_toml(
        r#"
[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = []
"#,
    )
    .unwrap();

    let profiles = anvil::cli::apply::resolve_profiles(&[], &manifest).unwrap();
    assert_eq!(profiles, vec!["base"]);
}

#[test]
fn test_resolve_profiles_no_default_errors() {
    let manifest = anvil::config::Manifest::parse_toml(
        r#"
[anvil]
version = "1"

[profiles.base]
links = []
"#,
    )
    .unwrap();

    let result = anvil::cli::apply::resolve_profiles(&[], &manifest);
    assert!(result.is_err());
}

#[test]
fn test_apply_specific_profile() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("dotfiles");
    let dest_base = dir.path().join("home").join(".zshrc");
    let dest_work = dir.path().join("home").join(".work_config");

    let manifest = format!(
        r#"
[anvil]
version = "1"

[profiles.base]
links = [
  {{ src = ".zshrc", dest = "{}" }},
]

[profiles.work]
links = [
  {{ src = ".work_config", dest = "{}" }},
]
"#,
        dest_base.display(),
        dest_work.display()
    );

    setup_repo(
        &repo,
        &manifest,
        &[(".zshrc", "# zshrc"), (".work_config", "# work")],
    );

    let manifest_parsed = anvil::config::Manifest::from_path(&repo.join("anvil.toml")).unwrap();
    let ctx = anvil::ui::UiContext::new(true, true, false);

    // Apply only "work" profile
    let profiles = vec!["work".to_string()];
    anvil::cli::apply::apply_profiles(&repo, &manifest_parsed, &profiles, &ctx).unwrap();

    // work config should be linked, base should not
    assert!(dest_work.exists());
    assert!(!dest_base.exists());
}
