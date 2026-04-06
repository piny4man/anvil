//! The `init` subcommand.
//!
//! Bootstraps anvil on a new machine: clones a dotfiles repo, parses its
//! manifest, applies links via the [`super::apply`] flow, and writes local
//! config so subsequent commands know where the repo lives.

use std::path::Path;

use crate::config::expand_tilde;
use crate::config::local::LocalConfig;
use crate::config::manifest::Manifest;
use crate::error::{AnvilError, Result};
use crate::git::{GitBackend, ShellGit};
use crate::ui::UiContext;

/// Entry point for `anvil init`. Prompts for URL and clone directory,
/// clones the repo, applies links, and writes local config.
pub fn run(url: Option<String>, profiles: Vec<String>, ctx: &UiContext) -> Result<()> {
    ctx.header();

    ShellGit::ensure_available()?;

    let url = match url {
        Some(u) => u,
        None => ctx.text("Repository URL:", None)?,
    };

    let default_dir = "~/.dotfiles";
    let clone_dir_raw = ctx.text("Clone to:", Some(default_dir))?;
    let clone_dir = expand_tilde(&clone_dir_raw)?;

    let resolved_profiles = run_init(&url, &clone_dir, &profiles, &ShellGit, ctx)?;

    LocalConfig::save(&clone_dir_raw, &resolved_profiles)?;
    ctx.success("Local config saved");

    Ok(())
}

/// Core init logic, factored out for testability. Accepts a git backend
/// and resolved paths so tests can inject mocks and temp directories.
///
/// Callers passing [`ShellGit`] should call [`ShellGit::ensure_available`]
/// first — this function does not check for git binary availability.
pub fn run_init(
    url: &str,
    clone_dir: &Path,
    explicit_profiles: &[String],
    git: &dyn GitBackend,
    ctx: &UiContext,
) -> Result<Vec<String>> {
    // Refuse to clone into an existing directory
    if clone_dir.is_dir() {
        return Err(AnvilError::Other(format!(
            "directory already exists: {}. Use `anvil sync` to update, or choose a different path.",
            clone_dir.display()
        )));
    }

    // Clone with spinner
    let spinner = ctx.spinner("Cloning repository\u{2026}");
    match git.clone_repo(url, clone_dir) {
        Ok(()) => {
            if let Some(s) = spinner {
                s.success("Repository cloned");
            }
        }
        Err(e) => {
            if let Some(s) = spinner {
                s.fail("Clone failed");
            }
            // Clean up partial clone
            let _ = std::fs::remove_dir_all(clone_dir);
            return Err(e);
        }
    }

    // Parse manifest
    let manifest_path = clone_dir.join("anvil.toml");
    let manifest = Manifest::from_path(&manifest_path)?;

    // Resolve profiles: --profile flags > manifest default
    let profile_names = resolve_init_profiles(explicit_profiles, &manifest)?;

    // Apply links
    super::apply::apply_profiles(clone_dir, &manifest, &profile_names, ctx)?;

    Ok(profile_names)
}

/// Resolves profiles for init. Unlike apply's resolver, init cannot
/// consult the local config (it doesn't exist yet).
fn resolve_init_profiles(explicit: &[String], manifest: &Manifest) -> Result<Vec<String>> {
    if !explicit.is_empty() {
        for name in explicit {
            manifest.get_profile(name)?;
        }
        return Ok(explicit.to_vec());
    }

    if let Some(default) = manifest.default_profile_name() {
        manifest.get_profile(default)?;
        return Ok(vec![default.to_string()]);
    }

    Err(AnvilError::Other(
        "no profile specified and no default_profile in anvil.toml. Use --profile <name>"
            .to_string(),
    ))
}
