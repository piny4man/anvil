//! Age-based secret decryption.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{AnvilError, Result};

/// Decrypt an age-encrypted file to bytes.
///
/// Prefers the `age` CLI if available (simpler dependency story for MVP).
/// Falls back to error with install hint.
pub fn decrypt_age(encrypted_path: &Path, identity_path: Option<&Path>) -> Result<Vec<u8>> {
    if !encrypted_path.exists() {
        return Err(AnvilError::Secrets(format!(
            "encrypted file not found: {}",
            encrypted_path.display()
        )));
    }

    if !age_cli_available() {
        return Err(AnvilError::Secrets(
            "`age` CLI not found; install age (https://github.com/FiloSottile/age) to decrypt secrets"
                .into(),
        ));
    }

    let mut cmd = Command::new("age");
    cmd.arg("-d");
    if let Some(id) = identity_path {
        cmd.arg("-i").arg(id);
    } else {
        // try default locations
        if let Some(home) = dirs::home_dir() {
            let default_id = home.join(".config/age/key.txt");
            if default_id.exists() {
                cmd.arg("-i").arg(default_id);
            }
        }
    }
    cmd.arg(encrypted_path);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd
        .output()
        .map_err(|e| AnvilError::Secrets(format!("failed to run age: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AnvilError::Secrets(format!("age decrypt failed: {err}")));
    }

    Ok(output.stdout)
}

fn age_cli_available() -> bool {
    Command::new("age")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Heuristic: flag likely plaintext secrets in a repo for doctor.
pub fn scan_repo_for_plaintext_secrets(repo: &Path) -> Vec<String> {
    let mut findings = Vec::new();
    let suspicious_names = [
        "id_rsa",
        "id_ed25519",
        ".env",
        "credentials.json",
        "secrets.yaml",
        "secrets.yml",
    ];

    let walker = walkdir_simple(repo, 4);
    for path in walker {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if suspicious_names
            .iter()
            .any(|s| name == *s || name.ends_with(s))
        {
            // allow .age encrypted
            if path.extension().and_then(|e| e.to_str()) == Some("age") {
                continue;
            }
            findings.push(path.display().to_string());
            continue;
        }
        // high-entropy-ish: look for PEM private keys
        if path.is_file()
            && let Ok(mut f) = fs::File::open(&path)
        {
            let mut buf = [0u8; 64];
            if f.read(&mut buf).is_ok() {
                let head = String::from_utf8_lossy(&buf);
                if head.contains("PRIVATE KEY") {
                    findings.push(path.display().to_string());
                }
            }
        }
    }
    findings
}

fn walkdir_simple(root: &Path, max_depth: usize) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, depth: usize, max: usize, out: &mut Vec<std::path::PathBuf>) {
        if depth > max {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" || name == "target" {
                continue;
            }
            if path.is_dir() {
                walk(&path, depth + 1, max, out);
            } else {
                out.push(path);
            }
        }
    }
    walk(root, 0, max_depth, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn decrypt_missing_file() {
        let err = decrypt_age(Path::new("/no/such/file.age"), None).unwrap_err();
        assert!(matches!(err, AnvilError::Secrets(_)));
    }

    #[test]
    fn scan_finds_pem() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("key.pem"),
            "-----BEGIN PRIVATE KEY-----\nABC\n",
        )
        .unwrap();
        let findings = scan_repo_for_plaintext_secrets(dir.path());
        assert!(!findings.is_empty());
    }
}
