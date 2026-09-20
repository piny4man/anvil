//! Personal Linux hardening checks (and optional enforce stubs).

use std::fs;
use std::process::Command;

use crate::config::{Firewall, Harden, SshHarden, SysctlEntry};
use crate::error::{AnvilError, Result};
use crate::linker::BackupJournal;
use crate::plan::HardenOp;
use crate::ui::UiContext;

/// Run all configured hardening checks (read-only).
pub fn check_all(harden: &Harden) -> Vec<HardenOp> {
    let mut ops = Vec::new();

    if let Some(sysctls) = &harden.sysctl {
        for entry in sysctls {
            ops.push(check_sysctl(entry));
        }
    }

    if let Some(ssh) = &harden.ssh {
        ops.extend(check_ssh(ssh));
    }

    if let Some(fw) = &harden.firewall {
        ops.push(check_firewall(fw));
    }

    // Always-on baseline checks when harden section present
    ops.push(check_home_permissions());

    ops
}

fn check_sysctl(entry: &SysctlEntry) -> HardenOp {
    let id = format!("sysctl:{}", entry.key);
    match read_sysctl(&entry.key) {
        Ok(current) => {
            let ok = current.trim() == entry.value.trim();
            HardenOp {
                id,
                description: format!("sysctl {}", entry.key),
                ok,
                detail: if ok {
                    format!("= {}", entry.value)
                } else {
                    format!("want {}, got {}", entry.value, current.trim())
                },
            }
        }
        Err(e) => HardenOp {
            id,
            description: format!("sysctl {}", entry.key),
            ok: false,
            detail: e,
        },
    }
}

fn read_sysctl(key: &str) -> std::result::Result<String, String> {
    // try /proc/sys first
    let path = format!("/proc/sys/{}", key.replace('.', "/"));
    if let Ok(v) = fs::read_to_string(&path) {
        return Ok(v);
    }
    let output = Command::new("sysctl")
        .arg("-n")
        .arg(key)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn check_ssh(ssh: &SshHarden) -> Vec<HardenOp> {
    let mut ops = Vec::new();
    let config = read_sshd_config();

    if let Some(want) = ssh.password_auth {
        let current = sshd_bool(&config, "PasswordAuthentication");
        // Default is yes when unset — fail if we want false and unset.
        let ok = match current {
            Some(v) => v == want,
            None => want,
        };
        ops.push(HardenOp {
            id: "ssh:password_auth".into(),
            description: "sshd PasswordAuthentication".into(),
            ok,
            detail: format!(
                "want {}, got {}",
                want,
                current
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "unset".into())
            ),
        });
    }

    if let Some(want_allow) = ssh.root_login {
        // PermitRootLogin can be yes/no/prohibit-password
        let raw = sshd_value(&config, "PermitRootLogin");
        let current_allows = match raw.as_deref() {
            Some("yes") => true,
            Some("no") | Some("prohibit-password") | Some("without-password") => false,
            _ => true, // default historically yes
        };
        let ok = current_allows == want_allow;
        ops.push(HardenOp {
            id: "ssh:root_login".into(),
            description: "sshd PermitRootLogin".into(),
            ok,
            detail: format!("want allow={want_allow}, raw={raw:?}"),
        });
    }

    ops
}

fn read_sshd_config() -> String {
    fs::read_to_string("/etc/ssh/sshd_config").unwrap_or_default()
}

fn sshd_value(config: &str, key: &str) -> Option<String> {
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        if parts.next()? == key {
            return parts.next().map(|s| s.to_string());
        }
    }
    None
}

fn sshd_bool(config: &str, key: &str) -> Option<bool> {
    sshd_value(config, key).and_then(|v| match v.to_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    })
}

fn check_firewall(fw: &Firewall) -> HardenOp {
    let backend = fw.backend.as_deref().unwrap_or("ufw");
    match backend {
        "ufw" => {
            let output = Command::new("ufw").arg("status").output();
            match output {
                Ok(o) if o.status.success() => {
                    let text = String::from_utf8_lossy(&o.stdout).to_lowercase();
                    let active = text.contains("active") && !text.contains("inactive");
                    HardenOp {
                        id: "firewall:ufw".into(),
                        description: "ufw firewall active".into(),
                        ok: active,
                        detail: if active {
                            "active".into()
                        } else {
                            "inactive or not configured".into()
                        },
                    }
                }
                Ok(_) | Err(_) => HardenOp {
                    id: "firewall:ufw".into(),
                    description: "ufw firewall active".into(),
                    ok: false,
                    detail: "ufw not available or not runnable".into(),
                },
            }
        }
        other => HardenOp {
            id: format!("firewall:{other}"),
            description: format!("firewall backend {other}"),
            ok: false,
            detail: "unsupported backend in this version".into(),
        },
    }
}

fn check_home_permissions() -> HardenOp {
    let Some(home) = dirs::home_dir() else {
        return HardenOp {
            id: "home:perms".into(),
            description: "home directory permissions".into(),
            ok: false,
            detail: "could not determine home".into(),
        };
    };
    match fs::metadata(&home) {
        Ok(meta) => {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o777;
            // world-writable or world-readable is bad for home
            let world = mode & 0o007;
            let ok = world == 0;
            HardenOp {
                id: "home:perms".into(),
                description: "home directory not world-accessible".into(),
                ok,
                detail: format!("mode {:o}", mode),
            }
        }
        Err(e) => HardenOp {
            id: "home:perms".into(),
            description: "home directory permissions".into(),
            ok: false,
            detail: e.to_string(),
        },
    }
}

/// Enforce hardening (only when mode=enforce). Currently supports writing sysctl.d drop-in.
pub fn enforce(
    harden: &Harden,
    ctx: &UiContext,
    journal: Option<&mut BackupJournal>,
) -> Result<()> {
    let mode = harden.mode.as_deref().unwrap_or("check");
    if mode != "enforce" {
        ctx.warn("harden.mode is not `enforce`; running checks only");
        return Ok(());
    }

    if ctx.dry_run {
        ctx.success("would enforce hardening settings (dry-run)");
        return Ok(());
    }

    let ok = ctx.confirm(
        "Enforce hardening changes? This may write sysctl drop-ins and require sudo.",
        false,
    )?;
    if !ok {
        ctx.warn("Skipped harden enforce");
        return Ok(());
    }

    if let Some(sysctls) = &harden.sysctl {
        enforce_sysctl(sysctls, ctx, journal)?;
    }

    // SSH/firewall enforce intentionally conservative: report only for now
    if harden.ssh.is_some() || harden.firewall.is_some() {
        ctx.warn(
            "SSH/firewall enforce is check-only in this version; apply recommended configs via links",
        );
    }

    Ok(())
}

fn enforce_sysctl(
    entries: &[SysctlEntry],
    ctx: &UiContext,
    journal: Option<&mut BackupJournal>,
) -> Result<()> {
    let mut body = String::from("# Managed by anvil\n");
    for e in entries {
        body.push_str(&format!("{} = {}\n", e.key, e.value));
    }
    let path = "/etc/sysctl.d/99-anvil.conf";
    if ctx.dry_run {
        ctx.success(&format!("would write {path}"));
        return Ok(());
    }

    if let Some(journal) = journal {
        journal.backup_sysctl_dropin(std::path::Path::new(path))?;
    }

    // write via sudo tee
    let status = Command::new("sudo")
        .arg("tee")
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(body.as_bytes())?;
            }
            child.wait()
        })
        .map_err(|e| AnvilError::Harden(e.to_string()))?;

    if !status.success() {
        return Err(AnvilError::Harden(format!(
            "failed to write {path}: {status}"
        )));
    }

    let _ = Command::new("sudo").args(["sysctl", "--system"]).status();
    ctx.success(&format!("Wrote {path} and reloaded sysctl"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_check_runs() {
        let op = check_home_permissions();
        assert_eq!(op.id, "home:perms");
    }

    #[test]
    fn sshd_parse() {
        let cfg = "# comment\nPasswordAuthentication no\nPermitRootLogin prohibit-password\n";
        assert_eq!(sshd_bool(cfg, "PasswordAuthentication"), Some(false));
        assert_eq!(
            sshd_value(cfg, "PermitRootLogin").as_deref(),
            Some("prohibit-password")
        );
    }
}
