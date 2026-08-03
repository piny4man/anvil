use crate::config::{LocalConfig, Manifest, discover_repo, resolve_profiles, select_profile_names};
use crate::error::{AnvilError, Result};
use crate::hooks;
use crate::linker::{BackupJournal, apply_link};
use crate::plan::{FileAction, build_plan, format_plan_summary};
use crate::secrets;
use crate::ui::UiContext;
use crate::ui::prompt::ConflictAction;
use crate::ui::summary::ApplySummary;

pub fn run(
    profiles: Vec<String>,
    with_packages: bool,
    with_harden: bool,
    ctx: &UiContext,
) -> Result<()> {
    let local = LocalConfig::load()?;
    let repo = discover_repo(&local)?;
    let manifest = Manifest::from_path(&repo.join("anvil.toml"))?;

    let hostname = crate::paths::hostname();
    let names = select_profile_names(&manifest, &profiles, &local.profiles, &hostname)?;
    let resolved = resolve_profiles(&manifest, &names)?;

    ctx.info(&format!("Profile: {}", names.join(" + ")));
    ctx.info(&format!("Repo:    {}", repo.display()));

    let plan = build_plan(&repo, &resolved, with_packages, with_harden)?;

    if !ctx.quiet {
        for line in format_plan_summary(&plan).lines() {
            ctx.info(line);
        }
    }

    if ctx.dry_run {
        for op in &plan.file_ops {
            let label = match op.action {
                FileAction::Create => "link",
                FileAction::Decrypt => "decrypt",
                FileAction::SkipCorrect => "ok",
                FileAction::Conflict => "conflict",
                FileAction::Broken => "broken",
            };
            ctx.info(&format!(
                "  [{label}] {} → {}",
                op.display_src, op.display_dest
            ));
        }
        if with_packages {
            crate::packages::install_missing(&plan.package_ops, ctx)?;
        }
        if with_harden && let Some(h) = &resolved.harden {
            for op in crate::harden::check_all(h) {
                if op.ok {
                    ctx.success(&format!("{} — {}", op.description, op.detail));
                } else {
                    ctx.warn(&format!("{} — {}", op.description, op.detail));
                }
            }
        }
        ctx.success("Dry-run complete (no changes made)");
        return Ok(());
    }

    // before hooks
    hooks::run_hooks(&repo, &plan.before_hooks, ctx)?;

    let mut journal = BackupJournal::create()?;
    let mut summary = ApplySummary::new();
    let identity = local.age_identity_expanded()?;

    for op in &plan.file_ops {
        match op.action {
            FileAction::SkipCorrect => {
                ctx.success(&format!("{} → already correct", op.display_dest));
                summary.skipped += 1;
            }
            FileAction::Broken => {
                ctx.error(&format!(
                    "{} → source missing ({})",
                    op.display_dest, op.display_src
                ));
                summary.failed += 1;
            }
            FileAction::Conflict => {
                let action = ctx.conflict_resolution(&op.link.dest)?;
                match action {
                    ConflictAction::Skip => {
                        ctx.warn(&format!("{} → skipped (conflict)", op.display_dest));
                        summary.skipped += 1;
                    }
                    ConflictAction::ShowDiff => {
                        show_diff(&op.link.src, &op.link.dest, ctx);
                        // re-ask once
                        let action2 = ctx.conflict_resolution(&op.link.dest)?;
                        if action2 == ConflictAction::Overwrite || ctx.force {
                            apply_one(op, &mut journal, identity.as_deref(), ctx, &mut summary)?;
                        } else {
                            ctx.warn(&format!("{} → skipped", op.display_dest));
                            summary.skipped += 1;
                        }
                    }
                    ConflictAction::Overwrite => {
                        apply_one(op, &mut journal, identity.as_deref(), ctx, &mut summary)?;
                    }
                }
            }
            FileAction::Create | FileAction::Decrypt => {
                apply_one(op, &mut journal, identity.as_deref(), ctx, &mut summary)?;
            }
        }
    }

    journal.save()?;

    hooks::run_hooks(&repo, &plan.after_hooks, ctx)?;

    if with_packages {
        crate::packages::install_missing(&plan.package_ops, ctx)?;
    }

    if with_harden && let Some(h) = &resolved.harden {
        for op in crate::harden::check_all(h) {
            if op.ok {
                ctx.success(&format!("{} — {}", op.description, op.detail));
            } else {
                ctx.warn(&format!("{} — {}", op.description, op.detail));
            }
        }
        crate::harden::enforce(h, ctx)?;
    }

    summary.print(ctx);

    if summary.failed > 0 {
        return Err(AnvilError::Other(format!(
            "{} file(s) failed",
            summary.failed
        )));
    }
    Ok(())
}

fn apply_one(
    op: &crate::plan::FileOp,
    journal: &mut BackupJournal,
    identity: Option<&std::path::Path>,
    ctx: &UiContext,
    summary: &mut ApplySummary,
) -> Result<()> {
    journal.backup_path(&op.link.dest)?;

    if let Some(backend) = &op.link.decrypt {
        if backend != "age" {
            return Err(AnvilError::Secrets(format!(
                "unsupported decrypt backend `{backend}`"
            )));
        }
        let data = secrets::decrypt_age(&op.link.src, identity)?;
        crate::linker::write_file(&op.link.dest, &data, op.link.mode, false)?;
        ctx.success(&format!("{} → decrypted", op.display_dest));
        summary.linked += 1;
        return Ok(());
    }

    apply_link(&op.link, false)?;
    let mode = if op.link.copy { "copied" } else { "symlinked" };
    ctx.success(&format!("{} → {mode}", op.display_dest));
    summary.linked += 1;
    Ok(())
}

fn show_diff(src: &std::path::Path, dest: &std::path::Path, ctx: &UiContext) {
    let output = std::process::Command::new("diff")
        .args(["-u", "--color=always"])
        .arg(dest)
        .arg(src)
        .output();
    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            if text.is_empty() {
                ctx.info("(no textual diff — binary or identical)");
            } else {
                for line in text.lines() {
                    ctx.info(line);
                }
            }
        }
        Err(_) => ctx.warn("diff command not available"),
    }
}
