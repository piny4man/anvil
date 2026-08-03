use std::process::Command;

use crate::config::{LocalConfig, Manifest, discover_repo, resolve_profiles, select_profile_names};
use crate::error::Result;
use crate::git::{GitBackend, ShellGit};
use crate::plan::build_plan;
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
    println!();

    let mut checks = Vec::new();

    // git
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

    // local config / repo
    let local = LocalConfig::load()?;
    let repo = discover_repo(&local);
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
                        if let Ok(plan) = build_plan(path, &resolved, false, false) {
                            let broken = plan
                                .file_ops
                                .iter()
                                .filter(|o| {
                                    matches!(
                                        o.action,
                                        crate::plan::FileAction::Broken
                                            | crate::plan::FileAction::Conflict
                                            | crate::plan::FileAction::Create
                                            | crate::plan::FileAction::Decrypt
                                    )
                                })
                                .count();
                            let total = plan.file_ops.len();
                            let healthy = total.saturating_sub(broken);
                            checks.push(Check {
                                name: "symlinks".into(),
                                ok: broken == 0,
                                detail: format!("{healthy}/{total} healthy"),
                                fix: if broken > 0 {
                                    Some("anvil apply --force".into())
                                } else {
                                    None
                                },
                            });
                        }

                        // harden
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

                        // secrets scan
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

    // age optional
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
            "not installed (optional)".into()
        },
        fix: if age_ok {
            None
        } else {
            Some("pacman -S age".into())
        },
    });

    // print
    let mut issues = 0;
    for c in &checks {
        if c.ok {
            if !ctx.quiet {
                println!(
                    "{INDENT}{} {:<20} {}",
                    style(SYMBOL_OK).green().bold(),
                    c.name,
                    style(&c.detail).dim()
                );
            }
        } else {
            issues += 1;
            println!(
                "{INDENT}{} {:<20} {}",
                style(SYMBOL_ERR).red().bold(),
                c.name,
                c.detail
            );
            if let Some(fix) = &c.fix {
                println!(
                    "{INDENT}  {} Fix: {fix}",
                    style(SYMBOL_WARN).yellow().bold()
                );
            }
        }
    }

    println!();
    if issues == 0 {
        ctx.success("All good!");
    } else {
        ctx.warn(&format!("{issues} issue(s) found."));
    }
    Ok(())
}
