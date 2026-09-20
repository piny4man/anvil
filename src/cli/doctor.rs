use std::process::Command;

use crate::config::{LocalConfig, Manifest, discover_repo, resolve_profiles, select_profile_names};
use crate::error::Result;
use crate::git::{GitBackend, ShellGit};
use crate::packages;
use crate::plan::{FileAction, build_plan};
use crate::secrets;
use crate::ui::UiContext;
use crate::ui::theme::{INDENT, SYMBOL_ERR, SYMBOL_OK, SYMBOL_WARN};
use console::style;

struct Check {
    name: String,
    ok: bool,
    detail: String,
    fix: Option<String>,
}

pub fn run(ctx: &UiContext) -> Result<()> {
    ctx.info("Checking anvil setup...");

    let mut checks = Vec::new();

    let git_ok = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let git_ver = Command::new("git")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    checks.push(Check {
        name: "git".into(),
        ok: git_ok,
        detail: if git_ok { git_ver } else { "not found".into() },
        fix: if git_ok {
            None
        } else {
            Some("install git".into())
        },
    });

    let local = LocalConfig::load()?;
    let repo = discover_repo(&local);
    let mut uses_age = false;
    let mut has_aur = false;
    match &repo {
        Ok(path) => {
            checks.push(Check {
                name: "clone dir".into(),
                ok: true,
                detail: path.display().to_string(),
                fix: None,
            });
            let manifest = Manifest::from_path(&path.join("anvil.toml"));
            match manifest {
                Ok(m) => {
                    checks.push(Check {
                        name: "anvil.toml".into(),
                        ok: true,
                        detail: "valid".into(),
                        fix: None,
                    });
                    let hostname = crate::paths::hostname();
                    let names = select_profile_names(&m, &[], &local.profiles, &hostname)
                        .unwrap_or_default();
                    if let Ok(resolved) = resolve_profiles(&m, &names) {
                        uses_age = resolved
                            .links
                            .iter()
                            .any(|l| l.decrypt.as_deref() == Some("age"));
                        has_aur = resolved
                            .packages
                            .aur
                            .as_ref()
                            .is_some_and(|v| !v.is_empty());

                        if let Ok(plan) = build_plan(path, &resolved, false, false) {
                            let unhealthy = plan
                                .file_ops
                                .iter()
                                .filter(|o| {
                                    matches!(o.action, FileAction::Broken | FileAction::Conflict)
                                })
                                .count();
                            let pending = plan
                                .file_ops
                                .iter()
                                .filter(|o| {
                                    matches!(o.action, FileAction::Create | FileAction::Decrypt)
                                })
                                .count();
                            let total = plan.file_ops.len();
                            let healthy = total.saturating_sub(unhealthy + pending);
                            let mut detail = format!("{healthy}/{total} healthy");
                            if pending > 0 {
                                detail.push_str(&format!(", {pending} pending"));
                            }
                            let fix = if unhealthy > 0 {
                                let dangling = plan.file_ops.iter().any(|o| {
                                    o.action == FileAction::Conflict
                                        && o.link.dest.symlink_metadata().ok().is_some_and(|m| {
                                            m.file_type().is_symlink() && !o.link.dest.exists()
                                        })
                                });
                                if dangling {
                                    Some(
                                        "dangling dest with src present: anvil apply --force"
                                            .into(),
                                    )
                                } else {
                                    Some("anvil apply --force".into())
                                }
                            } else if pending > 0 {
                                Some("anvil apply".into())
                            } else {
                                None
                            };
                            checks.push(Check {
                                name: "symlinks".into(),
                                ok: unhealthy == 0,
                                detail,
                                fix,
                            });
                        }

                        if let Some(h) = &resolved.harden {
                            for op in crate::harden::check_all(h) {
                                checks.push(Check {
                                    name: op.description.clone(),
                                    ok: op.ok,
                                    detail: op.detail,
                                    fix: if op.ok {
                                        None
                                    } else {
                                        Some("review harden section / anvil apply --harden".into())
                                    },
                                });
                            }
                        }

                        let findings = secrets::scan_repo_for_plaintext_secrets(path);
                        checks.push(Check {
                            name: "plaintext secrets".into(),
                            ok: findings.is_empty(),
                            detail: if findings.is_empty() {
                                "none detected".into()
                            } else {
                                format!("{} suspicious file(s)", findings.len())
                            },
                            fix: if findings.is_empty() {
                                None
                            } else {
                                Some(format!("encrypt with age: {}", findings.join(", ")))
                            },
                        });
                    }
                }
                Err(e) => {
                    checks.push(Check {
                        name: "anvil.toml".into(),
                        ok: false,
                        detail: e.to_string(),
                        fix: Some("fix anvil.toml in the repo root".into()),
                    });
                }
            }

            let git = ShellGit::new();
            if git.is_repo(path) {
                checks.push(Check {
                    name: "git repo".into(),
                    ok: true,
                    detail: "ok".into(),
                    fix: None,
                });
            }
        }
        Err(_) => {
            checks.push(Check {
                name: "clone dir".into(),
                ok: false,
                detail: "not configured".into(),
                fix: Some("anvil init <url>".into()),
            });
        }
    }

    if uses_age {
        let age_ok = Command::new("age")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        checks.push(Check {
            name: "age (secrets)".into(),
            ok: age_ok,
            detail: if age_ok {
                "found".into()
            } else {
                "not installed".into()
            },
            fix: if age_ok {
                None
            } else {
                Some("pacman -S age".into())
            },
        });
    }

    if has_aur {
        let helper = packages::resolve_aur_helper(None, local.aur_helper.as_deref());
        match helper {
            Ok(h) if !h.program.is_empty() => {
                checks.push(Check {
                    name: "AUR helper".into(),
                    ok: true,
                    detail: h.display,
                    fix: None,
                });
            }
            Ok(_) => {
                checks.push(Check {
                    name: "AUR helper".into(),
                    ok: false,
                    detail: "none found (auto)".into(),
                    fix: Some(
                        "install paru or yay, or set aur_helper in ~/.config/anvil/config.toml"
                            .into(),
                    ),
                });
            }
            Err(e) => {
                checks.push(Check {
                    name: "AUR helper".into(),
                    ok: false,
                    detail: e.to_string(),
                    fix: Some("set aur_helper to paru, yay, or anzen".into()),
                });
            }
        }
    }

    let mut issues = 0;
    for c in &checks {
        if c.ok {
            ctx.line(&format!(
                "{INDENT}{} {:<20} {}",
                style(SYMBOL_OK).green().bold(),
                c.name,
                style(&c.detail).dim()
            ));
        } else {
            issues += 1;
            ctx.line(&format!(
                "{INDENT}{} {:<20} {}",
                style(SYMBOL_ERR).red().bold(),
                c.name,
                c.detail
            ));
            if let Some(fix) = &c.fix {
                ctx.line(&format!(
                    "{INDENT}  {} Fix: {fix}",
                    style(SYMBOL_WARN).yellow().bold()
                ));
            }
        }
    }

    if issues == 0 {
        ctx.success("All good!");
    } else {
        ctx.warn(&format!("{issues} issue(s) found."));
    }
    Ok(())
}
