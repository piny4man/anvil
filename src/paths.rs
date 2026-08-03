//! Path expansion helpers.
//!
//! Always expand `~` via [`dirs::home_dir`] — never string-replace alone.

use std::path::{Path, PathBuf};

use crate::error::{AnvilError, Result};

/// Expand a user path.
///
/// Supports:
/// - `~` → home directory
/// - `~/foo` → home/foo
///
/// Rejects `~otheruser` (not supported). Absolute and relative paths pass through.
pub fn expand_path(input: &str) -> Result<PathBuf> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AnvilError::InvalidPath("empty path".into()));
    }

    if input == "~" {
        return dirs::home_dir().ok_or(AnvilError::HomeDirNotFound);
    }

    if let Some(rest) = input.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or(AnvilError::HomeDirNotFound)?;
        return Ok(home.join(rest));
    }

    if input.starts_with('~') {
        return Err(AnvilError::InvalidPath(format!(
            "unsupported tilde form `{input}` (only `~` and `~/...` are supported)"
        )));
    }

    Ok(PathBuf::from(input))
}

/// Expand if the path is relative to `base`, after tilde expansion.
pub fn expand_path_relative(input: &str, base: &Path) -> Result<PathBuf> {
    let expanded = expand_path(input)?;
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Ok(base.join(expanded))
    }
}

/// XDG config dir for anvil (`$XDG_CONFIG_HOME/anvil` or `~/.config/anvil`).
pub fn anvil_config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or(AnvilError::HomeDirNotFound)?;
    Ok(base.join("anvil"))
}

/// XDG state dir for anvil (`$XDG_STATE_HOME/anvil` or `~/.local/state/anvil`).
pub fn anvil_state_dir() -> Result<PathBuf> {
    // dirs 6 has state_dir on most platforms
    let base = dirs::state_dir()
        .or_else(|| dirs::data_local_dir().map(|p| p.join("state")))
        .ok_or(AnvilError::HomeDirNotFound)?;
    Ok(base.join("anvil"))
}

/// Backup journal root: `~/.local/state/anvil/backups`.
pub fn backups_dir() -> Result<PathBuf> {
    Ok(anvil_state_dir()?.join("backups"))
}

/// Current hostname (best-effort).
pub fn hostname() -> String {
    whoami::fallible::hostname().unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_absolute() {
        let p = expand_path("/tmp/foo").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn expand_tilde_slash() {
        let p = expand_path("~/.dotfiles").unwrap();
        assert!(p.ends_with(".dotfiles"));
        assert!(!p.to_string_lossy().contains('~'));
        assert!(p.is_absolute());
    }

    #[test]
    fn expand_bare_tilde() {
        let p = expand_path("~").unwrap();
        assert_eq!(p, dirs::home_dir().unwrap());
    }

    #[test]
    fn reject_other_user_tilde() {
        let err = expand_path("~alice/foo").unwrap_err();
        assert!(matches!(err, AnvilError::InvalidPath(_)));
    }

    #[test]
    fn reject_empty() {
        assert!(expand_path("").is_err());
        assert!(expand_path("   ").is_err());
    }

    #[test]
    fn relative_to_base() {
        let base = Path::new("/repo");
        let p = expand_path_relative(".zshrc", base).unwrap();
        assert_eq!(p, PathBuf::from("/repo/.zshrc"));
    }

    #[test]
    fn relative_absolute_stays() {
        let base = Path::new("/repo");
        let p = expand_path_relative("/etc/foo", base).unwrap();
        assert_eq!(p, PathBuf::from("/etc/foo"));
    }
}
