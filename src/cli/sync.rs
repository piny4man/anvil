//! The `sync` subcommand.
//!
//! Pulls the latest changes from the remote and re-applies links.
//! This is the daily-driver command: run it from anywhere to stay up to date.

use std::path::Path;

use crate::config::discover_repo;
use crate::config::manifest::Manifest;
use crate::error::Result;
use crate::git::{GitBackend, ShellGit};
use crate::ui::UiContext;

/// Entry point for `anvil sync`. Discovers the repo, pulls changes,
/// and re-applies all links.
pub fn run(ctx: &UiContext) -> Result<()> {
    ShellGit::ensure_available()?;
    let repo_dir = discover_repo()?;
    run_sync(&repo_dir, &ShellGit, ctx)
}

/// Core sync logic, factored out for testability. Accepts a git backend,
/// resolved repo path, and [`UiContext`] so tests can inject mocks, temp
/// directories, and UI overrides (quiet, dry-run, etc.).
pub fn run_sync(repo_dir: &Path, git: &dyn GitBackend, ctx: &UiContext) -> Result<()> {
    // Pull with spinner
    let spinner = ctx.spinner("Pulling latest changes\u{2026}");
    match git.pull(repo_dir) {
        Ok(result) => {
            if let Some(s) = spinner {
                if result.was_updated {
                    s.success(&result.summary);
                } else {
                    s.success("Already up to date");
                }
            }
        }
        Err(e) => {
            if let Some(s) = spinner {
                s.fail("Pull failed");
            }
            return Err(e);
        }
    }

    // Always re-apply links (even when up-to-date — links may have been broken)
    let manifest_path = repo_dir.join("anvil.toml");
    let manifest = Manifest::from_path(&manifest_path)?;
    let profile_names = super::apply::resolve_profiles(&[], &manifest)?;
    super::apply::apply_profiles(repo_dir, &manifest, &profile_names, ctx)?;

    Ok(())
}
