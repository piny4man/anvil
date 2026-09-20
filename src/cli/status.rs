use crate::config::{LocalConfig, Manifest, discover_repo, resolve_profiles, select_profile_names};
use crate::error::Result;
use crate::git::{GitBackend, ShellGit};
use crate::packages;
use crate::plan::{FileAction, build_plan};
use crate::ui::UiContext;
use crate::ui::theme::{INDENT, SYMBOL_ERR, SYMBOL_OK, SYMBOL_WARN};
use console::style;

pub fn run(profiles: Vec<String>, ctx: &UiContext) -> Result<()> {
    let local = LocalConfig::load()?;
    let repo = discover_repo(&local)?;
    let manifest = Manifest::from_path(&repo.join("anvil.toml"))?;
    let hostname = crate::paths::hostname();
    let names = select_profile_names(&manifest, &profiles, &local.profiles, &hostname)?;
    let resolved = resolve_profiles(&manifest, &names)?;
    let plan = build_plan(&repo, &resolved, true, true)?;

    let git = ShellGit::new();
    let remote = git
        .remote_url(&repo)
        .ok()
        .flatten()
        .unwrap_or_else(|| repo.display().to_string());
    let clean = git.status_clean(&repo).unwrap_or(true);
    let dirty = if clean { "clean" } else { "dirty" };

    ctx.line(&format!(
        "{INDENT}Profile: {}  (machine: {hostname})",
        names.join(" + ")
    ));
    ctx.line(&format!("{INDENT}Repo:    {remote}  ({dirty})"));

    let helper = packages::resolve_aur_helper(None, local.aur_helper.as_deref()).ok();
    if let Some(h) = &helper {
        ctx.line(&format!("{INDENT}AUR helper: {}", h.display));
    }

    let mut linked = Vec::new();
    let mut broken = Vec::new();
    let mut conflicts = Vec::new();
    let mut pending = Vec::new();

    for op in &plan.file_ops {
        match op.action {
            FileAction::SkipCorrect => linked.push(op),
            FileAction::Broken => broken.push(op),
            FileAction::Conflict => conflicts.push(op),
            FileAction::Create | FileAction::Decrypt => pending.push(op),
        }
    }

    if !linked.is_empty() {
        ctx.line(&format!("{INDENT}{}", style("LINKED").bold()));
        for op in &linked {
            ctx.line(&format!(
                "{INDENT}{} {} {} {}",
                style(SYMBOL_OK).green().bold(),
                op.display_dest,
                style("→").dim(),
                op.display_src
            ));
        }
    }

    if !pending.is_empty() {
        ctx.line(&format!("{INDENT}{}", style("PENDING").bold()));
        for op in &pending {
            ctx.line(&format!(
                "{INDENT}{} {} {} {}",
                style(SYMBOL_WARN).yellow().bold(),
                op.display_dest,
                style("→").dim(),
                op.display_src
            ));
        }
    }

    if !conflicts.is_empty() {
        ctx.line(&format!("{INDENT}{}", style("CONFLICT").bold()));
        for op in &conflicts {
            ctx.line(&format!(
                "{INDENT}{} {} {}",
                style(SYMBOL_WARN).yellow().bold(),
                op.display_dest,
                style("exists (not managed or differs)").dim()
            ));
        }
    }

    if !broken.is_empty() {
        ctx.line(&format!("{INDENT}{}", style("MISSING / BROKEN").bold()));
        for op in &broken {
            ctx.line(&format!(
                "{INDENT}{} {} {} {}",
                style(SYMBOL_ERR).red().bold(),
                op.display_dest,
                style("→").dim(),
                op.display_src
            ));
        }
    }

    let missing: Vec<_> = plan.packages_missing().collect();
    if !plan.package_ops.is_empty() {
        ctx.line(&format!("{INDENT}{}", style("PACKAGES").bold()));
        for p in &plan.package_ops {
            if p.installed {
                ctx.line(&format!(
                    "{INDENT}{} {} ({})",
                    style(SYMBOL_OK).green().bold(),
                    p.name,
                    p.manager
                ));
            } else {
                ctx.line(&format!(
                    "{INDENT}{} {} ({}) missing",
                    style(SYMBOL_ERR).red().bold(),
                    p.name,
                    p.manager
                ));
            }
        }
    }

    if !plan.harden_ops.is_empty() {
        ctx.line(&format!("{INDENT}{}", style("HARDEN").bold()));
        for h in &plan.harden_ops {
            if h.ok {
                ctx.line(&format!(
                    "{INDENT}{} {} — {}",
                    style(SYMBOL_OK).green().bold(),
                    h.description,
                    h.detail
                ));
            } else {
                ctx.line(&format!(
                    "{INDENT}{} {} — {}",
                    style(SYMBOL_WARN).yellow().bold(),
                    h.description,
                    h.detail
                ));
            }
        }
    }

    if broken.is_empty() && conflicts.is_empty() && missing.is_empty() {
        if pending.is_empty() {
            ctx.success("All good");
        } else {
            ctx.warn(&format!(
                "{} pending link(s) — run `anvil apply`",
                pending.len()
            ));
        }
    } else {
        ctx.warn(&format!(
            "{} issue(s): {} missing/broken, {} conflict(s), {} package(s) missing",
            broken.len() + conflicts.len() + missing.len(),
            broken.len(),
            conflicts.len(),
            missing.len()
        ));
    }

    Ok(())
}
