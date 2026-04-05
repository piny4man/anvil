use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AnvilError, Result};

/// Local anvil state stored at `~/.config/anvil.toml`.
///
/// This is anvil's own config — not the manifest inside the dotfiles repo.
/// Written by `init`, read by `sync` and `apply` to discover the repo.
#[derive(Debug, Deserialize)]
pub struct LocalConfig {
    pub clone_dir: String,
    #[serde(default)]
    pub profiles: Vec<String>,
}

impl LocalConfig {
    /// Returns the platform-specific path for the local config file.
    /// Uses `dirs::config_dir()` (e.g. `~/.config` on Linux).
    pub fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or(AnvilError::HomeDirNotFound)?;
        Ok(config_dir.join("anvil.toml"))
    }

    /// Reads the local config from disk. Returns `None` if the file doesn't exist.
    pub fn load() -> Result<Option<Self>> {
        let path = Self::path()?;
        Self::load_from(&path)
    }

    /// Reads from a specific path. Returns `None` if the file doesn't exist.
    pub fn load_from(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(path).map_err(|e| AnvilError::ConfigRead {
            path: path.to_path_buf(),
            source: e,
        })?;
        let config: Self =
            toml::from_str(&contents).map_err(|e| AnvilError::ConfigParse(e.to_string()))?;
        Ok(Some(config))
    }

    /// Resolves `clone_dir` to an absolute path with tilde expansion.
    pub fn clone_dir_expanded(&self) -> Result<PathBuf> {
        expand_tilde(&self.clone_dir)
    }

    /// Writes a local config to disk at the standard path.
    pub fn save(clone_dir: &str, profiles: &[String]) -> Result<()> {
        let path = Self::path()?;
        Self::save_to(&path, clone_dir, profiles)
    }

    /// Writes a local config to a specific path using `toml_edit` for clean output.
    pub fn save_to(path: &Path, clone_dir: &str, profiles: &[String]) -> Result<()> {
        use toml_edit::{Array, DocumentMut, value};

        let mut doc = DocumentMut::new();
        doc["clone_dir"] = value(clone_dir);

        let mut arr = Array::new();
        for p in profiles {
            arr.push(p.as_str());
        }
        doc["profiles"] = value(arr);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AnvilError::ConfigRead {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::write(path, doc.to_string()).map_err(|e| AnvilError::ConfigRead {
            path: path.to_path_buf(),
            source: e,
        })?;

        Ok(())
    }
}

/// Expands a leading `~/` to the user's home directory.
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or(AnvilError::HomeDirNotFound)?;
        Ok(home.join(rest))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Discovers the dotfiles repo directory.
///
/// Order: local config `clone_dir` -> `~/.dotfiles` fallback -> error.
pub fn discover_repo() -> Result<PathBuf> {
    discover_repo_with(LocalConfig::load()?)
}

/// Discovers repo from an already-loaded optional config.
pub fn discover_repo_with(config: Option<LocalConfig>) -> Result<PathBuf> {
    if let Some(cfg) = config {
        let dir = cfg.clone_dir_expanded()?;
        if dir.is_dir() {
            return Ok(dir);
        }
    }

    // Fallback: ~/.dotfiles
    let home = dirs::home_dir().ok_or(AnvilError::HomeDirNotFound)?;
    let fallback = home.join(".dotfiles");
    if fallback.is_dir() {
        return Ok(fallback);
    }

    Err(AnvilError::Other(
        "no dotfiles repo found. Run `anvil init <url>` to get started.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("anvil.toml");

        LocalConfig::save_to(&path, "~/.dotfiles", &["base".into(), "work".into()]).unwrap();

        let config = LocalConfig::load_from(&path).unwrap().unwrap();
        assert_eq!(config.clone_dir, "~/.dotfiles");
        assert_eq!(config.profiles, vec!["base", "work"]);
    }

    #[test]
    fn test_load_missing_file_returns_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let config = LocalConfig::load_from(&path).unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("anvil.toml");

        LocalConfig::save_to(&path, "/opt/dots", &[]).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/.dotfiles").unwrap();
        assert!(!expanded.to_string_lossy().contains('~'));
        assert!(expanded.ends_with(".dotfiles"));
    }

    #[test]
    fn test_expand_absolute_path() {
        let expanded = expand_tilde("/opt/dotfiles").unwrap();
        assert_eq!(expanded, PathBuf::from("/opt/dotfiles"));
    }

    #[test]
    fn test_discover_repo_from_config() {
        let dir = tempdir().unwrap();
        let repo_dir = dir.path().join("my-dots");
        std::fs::create_dir(&repo_dir).unwrap();

        let config = LocalConfig {
            clone_dir: repo_dir.to_string_lossy().to_string(),
            profiles: vec![],
        };
        let result = discover_repo_with(Some(config)).unwrap();
        assert_eq!(result, repo_dir);
    }

    #[test]
    fn test_discover_repo_no_config_no_fallback() {
        let result = discover_repo_with(None);
        // This may succeed if ~/.dotfiles exists on the system, or fail.
        // We test the error path specifically:
        if result.is_err() {
            let err = result.unwrap_err().to_string();
            assert!(err.contains("anvil init"));
        }
    }

    #[test]
    fn test_save_empty_profiles() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("anvil.toml");

        LocalConfig::save_to(&path, "~/.dotfiles", &[]).unwrap();

        let config = LocalConfig::load_from(&path).unwrap().unwrap();
        assert_eq!(config.clone_dir, "~/.dotfiles");
        assert!(config.profiles.is_empty());
    }
}
