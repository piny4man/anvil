use std::path::Path;

use crate::config::scaffold::write_starter_manifest;
use crate::config::{LocalConfig, Manifest};
use crate::error::{AnvilError, Result};
use crate::git::{GitBackend, ShellGit};
use crate::paths::expand_path;
use crate::ui::UiContext;

pub fn run(
    url: Option<String>,
    profiles: Vec<String>,
    dir: Option<String>,
    ctx: &UiContext,
) -> Result<()> {
    ctx.header();

    let url = match url {
        Some(u) => u,
        None => ctx.text("Dotfiles repo URL", None)?,
    };

    let clone_dir = match dir {
        Some(d) => expand_path(&d)?,
        None => {
            let default = "~/.dotfiles";
            let chosen = ctx.text("Clone into", Some(default))?;
            expand_path(&chosen)?
        }
    };

    let git = ShellGit::new();

    if clone_dir.exists() {
        if !git.is_repo(&clone_dir) {
            return Err(AnvilError::GitCloneFailed(format!(
                "destination exists and is not a git repository: {}",
                clone_dir.display()
            )));
        }
        ctx.warn(&format!(
            "{} already exists; reusing checkout",
            clone_dir.display()
        ));
    } else {
        let spinner = ctx.spinner("Cloning repository...");
        match git.clone_repo(&url, &clone_dir) {
            Ok(()) => {
                if let Some(s) = spinner {
                    s.success(&format!("Cloned into {}", clone_dir.display()));
                } else {
                    ctx.success(&format!("Cloned into {}", clone_dir.display()));
                }
            }
            Err(e) => {
                if let Some(s) = spinner {
                    s.fail("Clone failed");
                }
                return Err(e);
            }
        }
    }

    let manifest_path = clone_dir.join("anvil.toml");
    ensure_manifest(&clone_dir, &manifest_path, ctx)?;

    let manifest = Manifest::from_path(&manifest_path)?;

    let mut selected = profiles;
    if selected.is_empty() {
        let hostname = crate::paths::hostname();
        if let Some(machines) = &manifest.machines
            && let Some(p) = machines.get(&hostname)
        {
            selected = p.clone();
            ctx.success(&format!(
                "Using profiles for machine `{hostname}`: {}",
                selected.join(", ")
            ));
        }
    }

    if selected.is_empty() && !manifest.profiles.is_empty() {
        let mut names: Vec<String> = manifest.profiles.keys().cloned().collect();
        names.sort();
        if ctx.yes {
            if let Some(d) = manifest.default_profile_name() {
                selected = vec![d.to_string()];
            } else if names.contains(&"base".to_string()) {
                selected = vec!["base".into()];
            } else if let Some(first) = names.first() {
                selected = vec![first.clone()];
            }
            ctx.success(&format!("--yes: using profile(s) {}", selected.join(", ")));
        } else {
            let idxs = ctx.multi_select("Available profiles", names.clone())?;
            if idxs.is_empty() {
                if let Some(d) = manifest.default_profile_name() {
                    selected = vec![d.to_string()];
                } else if names.contains(&"base".to_string()) {
                    selected = vec!["base".into()];
                } else {
                    return Err(AnvilError::Other("no profiles selected".into()));
                }
            } else {
                selected = idxs.into_iter().map(|i| names[i].clone()).collect();
            }
        }
    }

    if selected.is_empty() {
        selected = vec!["base".into()];
    }

    // Save local config even when the repo is still empty of links
    let mut local = LocalConfig::load().unwrap_or_default();
    local.repo_path = Some(clone_dir.display().to_string());
    local.profiles = selected.clone();
    local.save()?;
    ctx.success(&format!(
        "Local config written to {}",
        LocalConfig::config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~/.config/anvil/config.toml".into())
    ));

    let link_count = manifest
        .profiles
        .values()
        .map(|p| p.links.len())
        .sum::<usize>();

    if link_count == 0 {
        ctx.warn("No links defined yet — repo is ready for bootstrap.");
        ctx.info("Next steps:");
        ctx.info(&format!("  1. cd {}", clone_dir.display()));
        ctx.info("  2. anvil add ~/.zshrc   # adopt existing configs");
        ctx.info("  3. git add anvil.toml && git commit -m \"chore: add anvil.toml\"");
        ctx.info("  4. anvil apply");
        ctx.success("Done! Empty profile applied (nothing to link).");
        return Ok(());
    }

    ctx.info(&format!("Applying profile: {}", selected.join(" + ")));
    crate::cli::apply::run(selected, false, false, ctx)?;

    ctx.success("Done! Run `anvil status` to see the current state.");
    Ok(())
}

/// Ensure `anvil.toml` exists; scaffold on first run when the repo is empty of anvil config.
fn ensure_manifest(repo: &Path, manifest_path: &Path, ctx: &UiContext) -> Result<()> {
    if manifest_path.exists() {
        return Ok(());
    }

    ctx.warn("No anvil.toml found in this repository.");

    let create = if ctx.yes {
        true
    } else {
        ctx.confirm(
            "Create a starter anvil.toml so you can bootstrap this machine?",
            true,
        )?
    };

    if !create {
        return Err(AnvilError::ManifestNotFound(manifest_path.to_path_buf()));
    }

    if ctx.dry_run {
        ctx.success(&format!(
            "would write starter anvil.toml to {}",
            manifest_path.display()
        ));
        // Still write in dry-run? No — but then later parse fails. For dry-run init,
        // write is the useful preview message and we stop before parse by using starter in memory…
        // Simpler: write even on dry_run only when user wants scaffold — actually respect dry_run.
        return Err(AnvilError::Other(
            "dry-run: would scaffold anvil.toml and finish init (re-run without --dry-run)".into(),
        ));
    }

    write_starter_manifest(repo)?;
    ctx.success(&format!(
        "Wrote starter anvil.toml → {}",
        manifest_path.display()
    ));
    ctx.info("Commit it when ready: git add anvil.toml && git commit");
    Ok(())
}
