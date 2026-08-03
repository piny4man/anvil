//! Starter `anvil.toml` for empty / new dotfiles repos.

use std::fs;
use std::path::Path;

use crate::error::{AnvilError, Result};

/// Minimal starter manifest written into a new or empty dots repo.
pub const STARTER_MANIFEST: &str = r#"# anvil.toml — managed by anvil
# Add files with: anvil add ~/.zshrc
# Docs: https://github.com/piny4man/anvil

[anvil]
version = "1"
default_profile = "base"

[profiles.base]
links = [
  # { src = ".zshrc", dest = "~/.zshrc" },
  # { src = ".config/nvim", dest = "~/.config/nvim" },
]

# packages.pacman = ["git", "age"]
# packages.aur = []

# [profiles.base.harden]
# mode = "check"
# ssh = { password_auth = false, root_login = false }
# firewall = { backend = "ufw" }

# [machines]
# "my-hostname" = ["base"]
"#;

/// Write a starter `anvil.toml` at `repo/anvil.toml` if missing.
pub fn write_starter_manifest(repo: &Path) -> Result<()> {
    let path = repo.join("anvil.toml");
    if path.exists() {
        return Ok(());
    }
    fs::write(&path, STARTER_MANIFEST).map_err(|e| AnvilError::Io {
        path: path.clone(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Manifest;
    use tempfile::tempdir;

    #[test]
    fn starter_parses() {
        let m = Manifest::parse_toml(STARTER_MANIFEST).unwrap();
        assert_eq!(m.anvil.version, "1");
        assert!(m.get_profile("base").is_ok());
    }

    #[test]
    fn write_once() {
        let dir = tempdir().unwrap();
        write_starter_manifest(dir.path()).unwrap();
        assert!(dir.path().join("anvil.toml").exists());
        // second write is no-op
        write_starter_manifest(dir.path()).unwrap();
    }
}
