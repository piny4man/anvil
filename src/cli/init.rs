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
        if git.is_repo(&clone_dir) && clone_dir.join("anvil.toml").exists() {
            ctx.warn(&format!(
                "{} already exists; writing local config and applying",
                clone_dir.display()
            ));
        } else {
            return Err(AnvilError::GitCloneFailed(format!(
                "destination exists and is not an anvil repo: {}",
                clone_dir.display()
            )));
        }
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
    let manifest = Manifest::from_path(&manifest_path)?;

    let mut selected = profiles;
    if selected.is_empty() {
        // machines table?
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
            // only default_profile under --yes
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
                } else {
                    return Err(AnvilError::Other("no profiles selected".into()));
                }
            } else {
                selected = idxs.into_iter().map(|i| names[i].clone()).collect();
            }
        }
    }

    // Save local config
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

    // Apply
    ctx.info(&format!("Applying profile: {}", selected.join(" + ")));
    crate::cli::apply::run(selected, false, false, ctx)?;

    ctx.success("Done! Run `anvil status` to see the current state.");
    Ok(())
}
