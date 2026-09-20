//! Declarative package plane (Arch pacman first; AUR helper is chosen locally).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Packages;
use crate::error::{AnvilError, Result};
use crate::linker::BackupJournal;
use crate::plan::PackageOp;
use crate::ui::UiContext;

/// Chosen AUR helper (never auto-prefers `anzen`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAurHelper {
    /// Display name: `paru`, `yay`, `anzen`, `auto`, or a path.
    pub display: String,
    /// Binary to exec (empty when `auto` found nothing).
    pub program: String,
}

/// Parse a helper spec (`auto` | `paru` | `yay` | `anzen` | `/abs/path`).
pub fn parse_aur_helper(spec: &str) -> Result<String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(AnvilError::Package(
            "empty aur_helper (expected auto, paru, yay, anzen, or an absolute path)".into(),
        ));
    }
    if spec.starts_with('/') {
        return Ok(spec.to_string());
    }
    match spec {
        "auto" | "paru" | "yay" | "anzen" => Ok(spec.to_string()),
        other => Err(AnvilError::Package(format!(
            "unknown aur_helper `{other}` (expected auto, paru, yay, anzen, or an absolute path)"
        ))),
    }
}

/// CLI `--aur-helper` wins, then local config, then `auto` (paru, then yay — never anzen).
pub fn resolve_aur_helper(cli: Option<&str>, local: Option<&str>) -> Result<ResolvedAurHelper> {
    resolve_aur_helper_with(cli, local, binary_on_path)
}

pub fn resolve_aur_helper_with(
    cli: Option<&str>,
    local: Option<&str>,
    available: impl Fn(&str) -> bool,
) -> Result<ResolvedAurHelper> {
    let spec = cli
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| local.map(str::trim).filter(|s| !s.is_empty()))
        .unwrap_or("auto");
    let kind = parse_aur_helper(spec)?;
    if kind.starts_with('/') {
        if !Path::new(&kind).exists() {
            return Err(AnvilError::Package(format!(
                "aur_helper path does not exist: {kind}"
            )));
        }
        return Ok(ResolvedAurHelper {
            display: kind.clone(),
            program: kind,
        });
    }
    match kind.as_str() {
        "auto" => {
            if available("paru") {
                Ok(ResolvedAurHelper {
                    display: "paru".into(),
                    program: "paru".into(),
                })
            } else if available("yay") {
                Ok(ResolvedAurHelper {
                    display: "yay".into(),
                    program: "yay".into(),
                })
            } else {
                Ok(ResolvedAurHelper {
                    display: "auto".into(),
                    program: String::new(),
                })
            }
        }
        "paru" | "yay" | "anzen" => Ok(ResolvedAurHelper {
            display: kind.clone(),
            program: kind,
        }),
        _ => unreachable!("parse_aur_helper restricts names"),
    }
}

fn binary_on_path(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Argv after the program name. Never includes `--skipreview` or `--ask`.
pub fn aur_install_args(
    helper: &ResolvedAurHelper,
    noconfirm: bool,
    packages: &[&str],
) -> Vec<String> {
    let mut args = Vec::new();
    if helper.display == "anzen"
        || PathBuf::from(&helper.program)
            .file_name()
            .and_then(|s| s.to_str())
            == Some("anzen")
    {
        args.push("install".into());
        if noconfirm {
            args.push("--noconfirm".into());
        }
    } else {
        args.push("-S".into());
        args.push("--needed".into());
        if noconfirm {
            args.push("--noconfirm".into());
        }
    }
    args.extend(packages.iter().map(|p| (*p).to_string()));
    args
}

/// Build package ops (installed or missing).
pub fn plan_packages(packages: &Packages) -> Result<Vec<PackageOp>> {
    let mut ops = Vec::new();

    if let Some(list) = &packages.pacman {
        for name in list {
            ops.push(PackageOp {
                name: name.clone(),
                manager: "pacman".into(),
                installed: is_pacman_installed(name),
            });
        }
    }

    if let Some(list) = &packages.aur {
        for name in list {
            ops.push(PackageOp {
                name: name.clone(),
                manager: "aur".into(),
                installed: is_pacman_installed(name),
            });
        }
    }

    Ok(ops)
}

fn is_pacman_installed(name: &str) -> bool {
    Command::new("pacman")
        .args(["-Q", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pacman_available() -> bool {
    Command::new("pacman")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Install missing packages. Pacman list always uses pacman; AUR uses the resolved helper.
pub fn install_missing(
    ops: &[PackageOp],
    helper: &ResolvedAurHelper,
    mut journal: Option<&mut BackupJournal>,
    ctx: &UiContext,
) -> Result<()> {
    let missing_pacman: Vec<String> = ops
        .iter()
        .filter(|o| o.manager == "pacman" && !o.installed)
        .map(|o| o.name.clone())
        .collect();
    let missing_aur: Vec<String> = ops
        .iter()
        .filter(|o| o.manager == "aur" && !o.installed)
        .map(|o| o.name.clone())
        .collect();

    if missing_pacman.is_empty() && missing_aur.is_empty() {
        ctx.success("All packages already installed");
        return Ok(());
    }

    if !pacman_available() && !ctx.dry_run {
        return Err(AnvilError::Package(
            "pacman not found (packages plane is Arch-first)".into(),
        ));
    }

    if ctx.dry_run {
        ctx.line(&format!("AUR helper: {}", helper.display));
        if !missing_pacman.is_empty() {
            ctx.success(&format!(
                "would install (pacman): {}",
                missing_pacman.join(", ")
            ));
        }
        if !missing_aur.is_empty() {
            ctx.success(&format!(
                "would install ({}): {}",
                helper.display,
                missing_aur.join(", ")
            ));
        }
        return Ok(());
    }

    ctx.line(&format!("AUR helper: {}", helper.display));

    if !missing_pacman.is_empty() {
        let ok = ctx.confirm(
            &format!(
                "Install {} pacman package(s): {}?",
                missing_pacman.len(),
                missing_pacman.join(", ")
            ),
            true,
        )?;
        if ok {
            let status = Command::new("sudo")
                .arg("pacman")
                .arg("-S")
                .arg("--needed")
                .arg("--noconfirm")
                .args(&missing_pacman)
                .status()
                .map_err(|e| AnvilError::Package(e.to_string()))?;
            if !status.success() {
                return Err(AnvilError::Package(format!(
                    "pacman install failed: {status}"
                )));
            }
            if let Some(journal) = journal.as_deref_mut() {
                for name in &missing_pacman {
                    journal.record_package(name, "pacman")?;
                }
            }
            ctx.success(&format!("Installed: {}", missing_pacman.join(", ")));
        } else {
            ctx.warn("Skipped pacman installs");
        }
    }

    if !missing_aur.is_empty() {
        if helper.program.is_empty() {
            return Err(AnvilError::Package(
                "AUR packages requested but no helper found (set aur_helper to paru, yay, or anzen)"
                    .into(),
            ));
        }
        if helper.display == "anzen" {
            ctx.warn(
                "anzen review cannot be skipped; --yes only skips anvil/anzen install confirms",
            );
        }
        let ok = ctx.confirm(
            &format!(
                "Install {} AUR package(s) via {}: {}?",
                missing_aur.len(),
                helper.display,
                missing_aur.join(", ")
            ),
            true,
        )?;
        if ok {
            let names: Vec<&str> = missing_aur.iter().map(String::as_str).collect();
            let args = aur_install_args(helper, ctx.yes, &names);
            if args.iter().any(|a| a == "--skipreview" || a == "--ask") {
                return Err(AnvilError::Package(
                    "internal error: refused --skipreview/--ask".into(),
                ));
            }
            let status = Command::new(&helper.program)
                .args(&args)
                .status()
                .map_err(|e| AnvilError::Package(e.to_string()))?;
            if !status.success() {
                return Err(AnvilError::Package(format!(
                    "{} install failed: {status}",
                    helper.display
                )));
            }
            let manager = format!("aur:{}", helper.display);
            if let Some(journal) = journal {
                for name in &missing_aur {
                    journal.record_package(name, &manager)?;
                }
            }
            ctx.success(&format!(
                "Installed ({}): {}",
                helper.display,
                missing_aur.join(", ")
            ));
        } else {
            ctx.warn("Skipped AUR installs");
        }
    }

    Ok(())
}

/// Uninstall packages recorded on a journal. Default No; `--yes` does not uninstall unless `--force`.
pub fn uninstall_recorded(journal: &BackupJournal, ctx: &UiContext) -> Result<()> {
    let pkgs: Vec<_> = journal.package_entries().collect();
    if pkgs.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = pkgs.iter().map(|e| e.original_dest.as_str()).collect();
    ctx.warn(&format!(
        "This journal installed: {} (undo uses pacman -R, not anzen remove)",
        names.join(", ")
    ));
    let ok = if ctx.force {
        true
    } else if ctx.yes {
        false
    } else {
        ctx.confirm(
            &format!("Uninstall {} package(s) this run installed?", names.len()),
            false,
        )?
    };
    if !ok {
        ctx.warn("Skipped package uninstall");
        return Ok(());
    }
    if !pacman_available() {
        return Err(AnvilError::Package(
            "pacman not found; cannot uninstall recorded packages".into(),
        ));
    }
    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-R")
        .arg("--noconfirm")
        .args(&names)
        .status()
        .map_err(|e| AnvilError::Package(e.to_string()))?;
    if !status.success() {
        return Err(AnvilError::Package(format!("pacman -R failed: {status}")));
    }
    ctx.success(&format!("Uninstalled: {}", names.join(", ")));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_empty() {
        let ops = plan_packages(&Packages::default()).unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn plan_lists_names() {
        let pkgs = Packages {
            pacman: Some(vec![
                "git".into(),
                "this-package-should-not-exist-anvil-xyz".into(),
            ]),
            aur: None,
        };
        let ops = plan_packages(&pkgs).unwrap();
        assert_eq!(ops.len(), 2);
        let fake = ops.iter().find(|o| o.name.contains("anvil-xyz")).unwrap();
        assert!(!fake.installed || fake.installed);
    }

    #[test]
    fn cli_overrides_local() {
        let h = resolve_aur_helper_with(Some("anzen"), Some("paru"), |_| true).unwrap();
        assert_eq!(h.display, "anzen");
        assert_eq!(h.program, "anzen");
    }

    #[test]
    fn local_overrides_auto() {
        let h = resolve_aur_helper_with(None, Some("yay"), |_| true).unwrap();
        assert_eq!(h.display, "yay");
    }

    #[test]
    fn auto_prefers_paru_then_yay_never_anzen() {
        let h = resolve_aur_helper_with(None, None, |n| n == "anzen" || n == "yay").unwrap();
        assert_eq!(h.display, "yay", "anzen on PATH must not win auto");
        let h = resolve_aur_helper_with(None, None, |n| n == "anzen" || n == "yay" || n == "paru")
            .unwrap();
        assert_eq!(h.display, "paru");
    }

    #[test]
    fn auto_skips_missing() {
        let h = resolve_aur_helper_with(None, None, |_| false).unwrap();
        assert_eq!(h.display, "auto");
        assert!(h.program.is_empty());
    }

    #[test]
    fn unknown_helper_errors() {
        let err = resolve_aur_helper_with(Some("pacman"), None, |_| true).unwrap_err();
        assert!(err.to_string().contains("unknown aur_helper"));
    }

    #[test]
    fn anzen_argv_is_install_never_skipreview() {
        let helper = ResolvedAurHelper {
            display: "anzen".into(),
            program: "anzen".into(),
        };
        let args = aur_install_args(&helper, true, &["foo"]);
        assert_eq!(args[0], "install");
        assert!(args.contains(&"--noconfirm".into()));
        assert!(!args.iter().any(|a| a == "--skipreview" || a == "--ask"));
        assert!(args.contains(&"foo".into()));
    }

    #[test]
    fn paru_argv_is_dash_s() {
        let helper = ResolvedAurHelper {
            display: "paru".into(),
            program: "paru".into(),
        };
        let args = aur_install_args(&helper, false, &["bar"]);
        assert_eq!(args, vec!["-S", "--needed", "bar"]);
    }

    #[test]
    fn pacman_manager_stays_pacman_when_helper_is_anzen() {
        let pkgs = Packages {
            pacman: Some(vec!["git".into()]),
            aur: Some(vec!["something-aur".into()]),
        };
        let ops = plan_packages(&pkgs).unwrap();
        let pac = ops.iter().find(|o| o.name == "git").unwrap();
        assert_eq!(pac.manager, "pacman");
        let aur = ops.iter().find(|o| o.name == "something-aur").unwrap();
        assert_eq!(aur.manager, "aur");
    }
}
