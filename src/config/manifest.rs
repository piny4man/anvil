use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AnvilError, Result};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub anvil: AnvilMeta,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    pub machines: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnvilMeta {
    pub version: String,
    pub default_profile: Option<String>,
    pub clone_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub extends: Option<String>,
    #[serde(default)]
    pub links: Vec<Link>,
    pub hooks: Option<Hooks>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub src: String,
    pub dest: String,
    pub copy: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    pub before_apply: Option<Vec<String>>,
    pub after_apply: Option<Vec<String>>,
}

impl Manifest {
    pub fn parse_toml(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| AnvilError::ConfigParse(e.to_string()))
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(AnvilError::ManifestNotFound(path.to_path_buf()));
        }
        let contents = std::fs::read_to_string(path).map_err(|e| AnvilError::ConfigRead {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::parse_toml(&contents)
    }

    pub fn default_profile_name(&self) -> Option<&str> {
        self.anvil.default_profile.as_deref()
    }

    pub fn get_profile(&self, name: &str) -> Result<&Profile> {
        self.profiles
            .get(name)
            .ok_or_else(|| AnvilError::ProfileNotFound(name.to_string()))
    }
}

impl AnvilMeta {
    pub fn clone_dir_or_default(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or(AnvilError::HomeDirNotFound)?;
        match &self.clone_dir {
            Some(dir) if dir.starts_with("~/") => Ok(home.join(&dir[2..])),
            Some(dir) => Ok(PathBuf::from(dir)),
            None => Ok(home.join(".dotfiles")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
[anvil]
version = "1"
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        assert_eq!(manifest.anvil.version, "1");
        assert!(manifest.profiles.is_empty());
        assert!(manifest.machines.is_none());
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r#"
[anvil]
version = "1"
default_profile = "base"
clone_dir = "~/.dotfiles"

[profiles.base]
links = [
  { src = ".zshrc", dest = "~/.zshrc" },
  { src = ".gitconfig", dest = "~/.gitconfig" },
]
hooks.after_apply = ["scripts/install.sh"]

[profiles.work]
extends = "base"
links = [
  { src = "work/.gitconfig", dest = "~/.gitconfig" },
]

[machines]
"framework-arch" = ["base"]
"work-macbook" = ["base", "work"]
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        assert_eq!(manifest.anvil.version, "1");
        assert_eq!(manifest.anvil.default_profile.as_deref(), Some("base"));
        assert_eq!(manifest.anvil.clone_dir.as_deref(), Some("~/.dotfiles"));
        assert_eq!(manifest.profiles.len(), 2);

        let base = manifest.get_profile("base").unwrap();
        assert_eq!(base.links.len(), 2);
        assert_eq!(base.links[0].src, ".zshrc");
        assert_eq!(base.links[0].dest, "~/.zshrc");
        assert!(base.extends.is_none());
        let hooks = base.hooks.as_ref().unwrap();
        assert_eq!(hooks.after_apply.as_ref().unwrap(), &["scripts/install.sh"]);

        let work = manifest.get_profile("work").unwrap();
        assert_eq!(work.extends.as_deref(), Some("base"));
        assert_eq!(work.links.len(), 1);

        let machines = manifest.machines.as_ref().unwrap();
        assert_eq!(machines["framework-arch"], vec!["base"]);
        assert_eq!(machines["work-macbook"], vec!["base", "work"]);
    }

    #[test]
    fn test_parse_rejects_unknown_fields() {
        let toml = r#"
[anvil]
version = "1"
bogus_field = "oops"
"#;
        let err = Manifest::parse_toml(toml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown field"), "got: {msg}");
    }

    #[test]
    fn test_parse_rejects_unknown_link_fields() {
        let toml = r#"
[anvil]
version = "1"

[profiles.base]
links = [
  { src = ".zshrc", dest = "~/.zshrc", secret = true },
]
"#;
        let err = Manifest::parse_toml(toml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown field"), "got: {msg}");
    }

    #[test]
    fn test_parse_missing_version() {
        let toml = r#"
[anvil]
default_profile = "base"
"#;
        assert!(Manifest::parse_toml(toml).is_err());
    }

    #[test]
    fn test_parse_empty_links() {
        let toml = r#"
[anvil]
version = "1"

[profiles.base]
links = []
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        let base = manifest.get_profile("base").unwrap();
        assert!(base.links.is_empty());
    }

    #[test]
    fn test_get_profile_found() {
        let toml = r#"
[anvil]
version = "1"

[profiles.base]
links = []
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        assert!(manifest.get_profile("base").is_ok());
    }

    #[test]
    fn test_get_profile_not_found() {
        let toml = r#"
[anvil]
version = "1"
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        let err = manifest.get_profile("nonexistent").unwrap_err();
        assert!(matches!(err, AnvilError::ProfileNotFound(ref name) if name == "nonexistent"));
    }

    #[test]
    fn test_clone_dir_default() {
        let meta = AnvilMeta {
            version: "1".to_string(),
            default_profile: None,
            clone_dir: None,
        };
        let path = meta.clone_dir_or_default().unwrap();
        assert!(path.ends_with(".dotfiles"));
    }

    #[test]
    fn test_clone_dir_tilde_expansion() {
        let meta = AnvilMeta {
            version: "1".to_string(),
            default_profile: None,
            clone_dir: Some("~/.config/dots".to_string()),
        };
        let path = meta.clone_dir_or_default().unwrap();
        assert!(path.ends_with(".config/dots"));
        assert!(!path.to_string_lossy().contains('~'));
    }

    #[test]
    fn test_clone_dir_absolute_path() {
        let meta = AnvilMeta {
            version: "1".to_string(),
            default_profile: None,
            clone_dir: Some("/opt/dotfiles".to_string()),
        };
        let path = meta.clone_dir_or_default().unwrap();
        assert_eq!(path, PathBuf::from("/opt/dotfiles"));
    }

    #[test]
    fn test_default_profile_name() {
        let toml = r#"
[anvil]
version = "1"
default_profile = "base"
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        assert_eq!(manifest.default_profile_name(), Some("base"));
    }

    #[test]
    fn test_default_profile_name_none() {
        let toml = r#"
[anvil]
version = "1"
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        assert_eq!(manifest.default_profile_name(), None);
    }

    #[test]
    fn test_link_copy_field() {
        let toml = r#"
[anvil]
version = "1"

[profiles.base]
links = [
  { src = ".bashrc", dest = "~/.bashrc", copy = true },
  { src = ".zshrc", dest = "~/.zshrc" },
]
"#;
        let manifest = Manifest::parse_toml(toml).unwrap();
        let base = manifest.get_profile("base").unwrap();
        assert_eq!(base.links[0].copy, Some(true));
        assert_eq!(base.links[1].copy, None);
    }
}
