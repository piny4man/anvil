//! Machine-local anvil configuration (not shared via git).
//!
//! Lives at `~/.config/anvil/config.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AnvilError, Result};
use crate::paths::{anvil_config_dir, expand_path};

const CONFIG_FILE: &str = "config.toml";

/// Local, per-machine settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalConfig {
    /// Absolute path to the dotfiles repository checkout.
    pub repo_path: Option<String>,
    /// Active profiles for this machine (override `[machines]` when set).
    #[serde(default)]
    pub profiles: Vec<String>,
    /// Path to age identity file for decrypting secrets.
    pub age_identity: Option<String>,
    /// AUR helper: `auto` | `paru` | `yay` | `anzen` | absolute path. Not shared via git.
    #[serde(default)]
    pub aur_helper: Option<String>,
}

impl LocalConfig {
    pub fn config_path() -> Result<PathBuf> {
        Ok(anvil_config_dir()?.join(CONFIG_FILE))
    }

    /// Load local config if it exists; otherwise return defaults.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path).map_err(|e| AnvilError::ConfigRead {
            path: path.clone(),
            source: e,
        })?;
        toml::from_str(&contents).map_err(|e| AnvilError::ConfigParse(e.to_string()))
    }

    /// Load from an explicit path (tests).
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path).map_err(|e| AnvilError::ConfigRead {
            path: path.to_path_buf(),
            source: e,
        })?;
        toml::from_str(&contents).map_err(|e| AnvilError::ConfigParse(e.to_string()))
    }

    /// Persist local config (creates parent dirs).
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| AnvilError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let contents =
            toml::to_string_pretty(self).map_err(|e| AnvilError::Other(e.to_string()))?;
        fs::write(path, contents).map_err(|e| AnvilError::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// Resolved absolute repo path, if configured.
    pub fn repo_path_expanded(&self) -> Result<Option<PathBuf>> {
        match &self.repo_path {
            Some(p) => Ok(Some(expand_path(p)?)),
            None => Ok(None),
        }
    }

    /// Require a configured repo or error.
    pub fn require_repo(&self) -> Result<PathBuf> {
        self.repo_path_expanded()?
            .filter(|p| p.exists())
            .ok_or(AnvilError::RepoNotConfigured)
    }

    pub fn age_identity_expanded(&self) -> Result<Option<PathBuf>> {
        match &self.age_identity {
            Some(p) => Ok(Some(expand_path(p)?)),
            None => Ok(None),
        }
    }
}

/// Discover the dotfiles repo: local config → default `~/.dotfiles` if it has anvil.toml.
pub fn discover_repo(local: &LocalConfig) -> Result<PathBuf> {
    if let Some(path) = local.repo_path_expanded()? {
        if path.join("anvil.toml").exists() {
            return Ok(path);
        }
        if path.exists() {
            return Ok(path);
        }
    }

    let default = expand_path("~/.dotfiles")?;
    if default.join("anvil.toml").exists() {
        return Ok(default);
    }

    Err(AnvilError::RepoNotConfigured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_local_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = LocalConfig {
            repo_path: Some("/tmp/dots".into()),
            profiles: vec!["base".into(), "hyprland".into()],
            age_identity: Some("~/.config/age/key.txt".into()),
            aur_helper: Some("paru".into()),
        };
        cfg.save_to(&path).unwrap();
        let loaded = LocalConfig::load_from(&path).unwrap();
        assert_eq!(loaded.repo_path.as_deref(), Some("/tmp/dots"));
        assert_eq!(loaded.profiles, vec!["base", "hyprland"]);
    }

    #[test]
    fn load_missing_is_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.toml");
        let cfg = LocalConfig::load_from(&path).unwrap();
        assert!(cfg.repo_path.is_none());
        assert!(cfg.profiles.is_empty());
    }

    #[test]
    fn reject_unknown_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "repo_path = \"/x\"\nbogus = 1\n").unwrap();
        assert!(LocalConfig::load_from(&path).is_err());
    }

    #[test]
    fn accepts_aur_helper_field() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "aur_helper = \"anzen\"\n").unwrap();
        let cfg = LocalConfig::load_from(&path);
        assert!(
            cfg.is_ok(),
            "aur_helper is a valid local config field: {:?}",
            cfg.err()
        );
    }
}
