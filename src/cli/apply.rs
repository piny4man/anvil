//! The `apply` subcommand.
//!
//! Reads the dotfiles manifest, resolves which profiles to apply, and
//! creates symlinks (or copies) for every link entry. Handles conflict
//! resolution interactively and prints a coloured summary when finished.
//! The core [`apply_profiles`] function is also called by `init` and `sync`.

use std::path::{Path, PathBuf};

use console::style;

use crate::config::manifest::Manifest;
use crate::config::{LocalConfig, discover_repo};
use crate::error::{AnvilError, Result};
use crate::linker::{LinkResult, ResolvedLink, process_link};
use crate::ui::UiContext;
use crate::ui::prompt::ConflictAction;
use crate::ui::summary::ApplySummary;
use crate::ui::theme::SYMBOL_ARROW;

/// Entry point for `anvil apply`. Discovers the repo, loads the manifest,
/// resolves profiles, and applies all links.
pub fn run(profiles: Vec<String>, ctx: &UiContext) -> Result<()> {
    let repo_dir = discover_repo()?;
    let manifest_path = repo_dir.join("anvil.toml");
    let manifest = Manifest::from_path(&manifest_path)?;

    let profile_names = resolve_profiles(&profiles, &manifest)?;

    apply_profiles(&repo_dir, &manifest, &profile_names, ctx)
}

/// Runs the apply flow for a given repo directory and manifest.
/// Used by `init` and `sync` as well.
pub fn apply_profiles(
    repo_dir: &Path,
    manifest: &Manifest,
    profile_names: &[String],
    ctx: &UiContext,
) -> Result<()> {
    let mut summary = ApplySummary::new();

    for profile_name in profile_names {
        let profile = manifest.get_profile(profile_name)?;

        if profile.links.is_empty() {
            continue;
        }

        // Resolve all links to absolute paths
        let links: Vec<ResolvedLink> = profile
            .links
            .iter()
            .map(|l| ResolvedLink::resolve(repo_dir, &l.src, &l.dest, l.copy.unwrap_or(false)))
            .collect::<Result<Vec<_>>>()?;

        for link in &links {
            let result = process_link(link, ctx.dry_run);

            match result {
                LinkResult::Linked => {
                    summary.linked += 1;
                    ctx.info(&format!(
                        "{} {} {} {}",
                        style("linked").green(),
                        link.dest.display(),
                        SYMBOL_ARROW,
                        link.src.display()
                    ));
                }
                LinkResult::AlreadyCorrect => {
                    summary.skipped += 1;
                    ctx.info(&format!(
                        "{} {} (already correct)",
                        style("skip").dim(),
                        link.dest.display()
                    ));
                }
                LinkResult::Conflict => {
                    handle_conflict(link, &mut summary, ctx)?;
                }
                LinkResult::Failed(msg) => {
                    summary.failed += 1;
                    ctx.warn(&msg);
                }
                LinkResult::Ready => unreachable!("process_link never returns Ready"),
            }
        }
    }

    summary.print(ctx);
    Ok(())
}

/// Handles a conflict: file exists at dest and is not managed by anvil.
fn handle_conflict(link: &ResolvedLink, summary: &mut ApplySummary, ctx: &UiContext) -> Result<()> {
    let action = ctx.conflict_resolution(&link.dest)?;

    match action {
        ConflictAction::Skip => {
            summary.skipped += 1;
            ctx.info(&format!(
                "{} {} (skipped, file exists)",
                style("skip").yellow(),
                link.dest.display()
            ));
        }
        ConflictAction::Overwrite => {
            if !ctx.dry_run {
                // Backup existing file by appending .bak to the full path
                let backup = PathBuf::from(format!("{}.bak", link.dest.display()));
                std::fs::rename(&link.dest, &backup).map_err(|e| AnvilError::SymlinkFailed {
                    path: link.dest.clone(),
                    source: e,
                })?;

                let result = link.apply();
                match result {
                    LinkResult::Linked => {
                        summary.linked += 1;
                        ctx.info(&format!(
                            "{} {} {} {} (overwrote, backup at {})",
                            style("linked").green(),
                            link.dest.display(),
                            SYMBOL_ARROW,
                            link.src.display(),
                            backup.display()
                        ));
                    }
                    LinkResult::Failed(msg) => {
                        summary.failed += 1;
                        ctx.warn(&msg);
                    }
                    _ => {}
                }
            } else {
                summary.linked += 1;
                ctx.info(&format!(
                    "{} {} (would overwrite)",
                    style("linked").cyan(),
                    link.dest.display()
                ));
            }
        }
        ConflictAction::ShowDiff => {
            // Phase 4 will add actual diff rendering.
            summary.skipped += 1;
            ctx.info(&format!(
                "{} {} (diff not yet implemented, skipping)",
                style("skip").yellow(),
                link.dest.display()
            ));
        }
    }

    Ok(())
}

/// Determines which profiles to apply.
///
/// Priority: `--profile` flags > local config > manifest default.
pub fn resolve_profiles(explicit: &[String], manifest: &Manifest) -> Result<Vec<String>> {
    if !explicit.is_empty() {
        // Validate all requested profiles exist
        for name in explicit {
            manifest.get_profile(name)?;
        }
        return Ok(explicit.to_vec());
    }

    // Try local config
    if let Some(config) = LocalConfig::load()?
        && !config.profiles.is_empty()
    {
        return Ok(config.profiles);
    }

    // Fall back to manifest default
    if let Some(default) = manifest.default_profile_name() {
        manifest.get_profile(default)?;
        return Ok(vec![default.to_string()]);
    }

    Err(AnvilError::Other(
        "no profile specified. Use --profile <name> or set default_profile in anvil.toml"
            .to_string(),
    ))
}
