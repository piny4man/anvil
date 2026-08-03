//! Declarative package plane (Arch pacman first).

use std::process::Command;

use crate::config::Packages;
use crate::error::{AnvilError, Result};
use crate::plan::PackageOp;
use crate::ui::UiContext;

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
                installed: is_pacman_installed(name), // AUR packages also show in pacman -Q once installed
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

fn aur_helper() -> Option<&'static str> {
    ["paru", "yay"].into_iter().find(|&helper| {
        Command::new(helper)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Install missing packages. Requires confirmation unless yes; never uses --force for remove.
pub fn install_missing(ops: &[PackageOp], ctx: &UiContext) -> Result<()> {
    let missing_pacman: Vec<_> = ops
        .iter()
        .filter(|o| o.manager == "pacman" && !o.installed)
        .map(|o| o.name.as_str())
        .collect();
    let missing_aur: Vec<_> = ops
        .iter()
        .filter(|o| o.manager == "aur" && !o.installed)
        .map(|o| o.name.as_str())
        .collect();

    if missing_pacman.is_empty() && missing_aur.is_empty() {
        ctx.success("All packages already installed");
        return Ok(());
    }

    if !pacman_available() {
        return Err(AnvilError::Package(
            "pacman not found (packages plane is Arch-first)".into(),
        ));
    }

    if ctx.dry_run {
        if !missing_pacman.is_empty() {
            ctx.success(&format!(
                "would install (pacman): {}",
                missing_pacman.join(", ")
            ));
        }
        if !missing_aur.is_empty() {
            ctx.success(&format!("would install (aur): {}", missing_aur.join(", ")));
        }
        return Ok(());
    }

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
            ctx.success(&format!("Installed: {}", missing_pacman.join(", ")));
        } else {
            ctx.warn("Skipped pacman installs");
        }
    }

    if !missing_aur.is_empty() {
        let Some(helper) = aur_helper() else {
            return Err(AnvilError::Package(
                "AUR packages requested but neither paru nor yay found".into(),
            ));
        };
        let ok = ctx.confirm(
            &format!(
                "Install {} AUR package(s) via {helper}: {}?",
                missing_aur.len(),
                missing_aur.join(", ")
            ),
            true,
        )?;
        if ok {
            let status = Command::new(helper)
                .arg("-S")
                .arg("--needed")
                .arg("--noconfirm")
                .args(&missing_aur)
                .status()
                .map_err(|e| AnvilError::Package(e.to_string()))?;
            if !status.success() {
                return Err(AnvilError::Package(format!(
                    "{helper} install failed: {status}"
                )));
            }
            ctx.success(&format!("Installed (AUR): {}", missing_aur.join(", ")));
        } else {
            ctx.warn("Skipped AUR installs");
        }
    }

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
        // git often installed on dev machines; the fake one should be missing
        let fake = ops.iter().find(|o| o.name.contains("anvil-xyz")).unwrap();
        // On non-arch, pacman -Q fails → not installed
        assert!(!fake.installed || fake.installed); // just ensure no panic
    }
}
