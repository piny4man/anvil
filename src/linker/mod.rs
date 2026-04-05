//! Symlink and copy engine for anvil.
//!
//! Resolves manifest link entries into absolute paths, checks the filesystem
//! state of each destination, and creates symlinks (or copies in copy-mode).
//! All conflict detection lives here; conflict *resolution* is handled by the
//! caller (typically [`crate::cli::apply`]).

use std::path::{Path, PathBuf};

use crate::config::expand_tilde;
use crate::error::Result;

/// A fully resolved link with absolute paths, ready for filesystem operations.
#[derive(Debug, Clone)]
pub struct ResolvedLink {
    /// Absolute path to the source file inside the dotfiles repo.
    pub src: PathBuf,
    /// Absolute path to the destination on the system.
    pub dest: PathBuf,
    /// If true, copy the file instead of creating a symlink.
    pub copy: bool,
}

/// The outcome of checking or processing a single link.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkResult {
    /// Destination absent — ready to be linked.
    Ready,
    /// Symlink/copy was created successfully.
    Linked,
    /// Destination already points to the correct source — no-op.
    AlreadyCorrect,
    /// Destination exists and is not managed by anvil.
    Conflict,
    /// An error occurred.
    Failed(String),
}

impl ResolvedLink {
    /// Resolves a link entry from the manifest into absolute paths.
    ///
    /// `src` is relative to `repo_dir`, `dest` gets tilde expansion.
    pub fn resolve(repo_dir: &Path, src: &str, dest: &str, copy: bool) -> Result<Self> {
        let abs_src = repo_dir.join(src);
        let abs_dest = expand_tilde(dest)?;
        Ok(Self {
            src: abs_src,
            dest: abs_dest,
            copy,
        })
    }

    /// Checks the current state of this link on the filesystem.
    pub fn check(&self) -> LinkResult {
        if !self.dest.exists() && self.dest.symlink_metadata().is_err() {
            return LinkResult::Ready;
        }

        // Check if it's already a symlink pointing to the right place
        if let Ok(meta) = self.dest.symlink_metadata()
            && meta.is_symlink()
            && let Ok(target) = std::fs::read_link(&self.dest)
            && target == self.src
        {
            return LinkResult::AlreadyCorrect;
        }

        // Something else exists at dest
        LinkResult::Conflict
    }

    /// Creates the symlink or copy. Returns the outcome.
    ///
    /// Assumes conflict resolution has already been handled if needed.
    pub fn apply(&self) -> LinkResult {
        if !self.src.exists() {
            return LinkResult::Failed(format!("source file not found: {}", self.src.display()));
        }

        // Create parent directories
        if let Some(parent) = self.dest.parent()
            && !parent.exists()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return LinkResult::Failed(format!(
                "failed to create directory {}: {e}",
                parent.display()
            ));
        }

        if self.copy {
            self.apply_copy()
        } else {
            self.apply_symlink()
        }
    }

    fn apply_symlink(&self) -> LinkResult {
        #[cfg(unix)]
        {
            if let Err(e) = std::os::unix::fs::symlink(&self.src, &self.dest) {
                return LinkResult::Failed(format!(
                    "failed to symlink {} -> {}: {e}",
                    self.dest.display(),
                    self.src.display()
                ));
            }
        }

        #[cfg(not(unix))]
        {
            return LinkResult::Failed("symlinks not supported on this platform".to_string());
        }

        LinkResult::Linked
    }

    fn apply_copy(&self) -> LinkResult {
        match std::fs::copy(&self.src, &self.dest) {
            Ok(_) => LinkResult::Linked,
            Err(e) => LinkResult::Failed(format!(
                "failed to copy {} -> {}: {e}",
                self.src.display(),
                self.dest.display()
            )),
        }
    }
}

/// Processes a single link: check state, apply if needed.
///
/// Returns `LinkResult` indicating what happened. Does NOT handle conflicts —
/// caller must check for `Conflict` and resolve before calling `apply`.
pub fn process_link(link: &ResolvedLink, dry_run: bool) -> LinkResult {
    let state = link.check();

    match state {
        LinkResult::Ready => {
            if dry_run {
                LinkResult::Linked
            } else {
                link.apply()
            }
        }
        LinkResult::AlreadyCorrect => LinkResult::AlreadyCorrect,
        LinkResult::Conflict => LinkResult::Conflict,
        LinkResult::Linked | LinkResult::Failed(_) => state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_link() {
        let repo = PathBuf::from("/home/user/.dotfiles");
        let link = ResolvedLink::resolve(&repo, ".zshrc", "/home/user/.zshrc", false).unwrap();
        assert_eq!(link.src, PathBuf::from("/home/user/.dotfiles/.zshrc"));
        assert_eq!(link.dest, PathBuf::from("/home/user/.zshrc"));
        assert!(!link.copy);
    }

    #[test]
    fn test_resolve_link_with_tilde() {
        let repo = PathBuf::from("/tmp/repo");
        let link = ResolvedLink::resolve(&repo, "config", "~/.config/app", false).unwrap();
        assert!(!link.dest.to_string_lossy().contains('~'));
        assert!(link.dest.ends_with(".config/app"));
    }

    #[test]
    fn test_creates_symlink() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("link.txt");

        std::fs::write(&src, "hello").unwrap();

        let link = ResolvedLink {
            src: src.clone(),
            dest: dest.clone(),
            copy: false,
        };

        let result = link.apply();
        assert_eq!(result, LinkResult::Linked);
        assert!(dest.symlink_metadata().unwrap().is_symlink());
        assert_eq!(std::fs::read_link(&dest).unwrap(), src);
    }

    #[test]
    fn test_detects_correct_symlink() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("link.txt");

        std::fs::write(&src, "hello").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&src, &dest).unwrap();

        let link = ResolvedLink {
            src: src.clone(),
            dest: dest.clone(),
            copy: false,
        };

        assert_eq!(link.check(), LinkResult::AlreadyCorrect);
    }

    #[test]
    fn test_detects_conflict() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("existing.txt");

        std::fs::write(&src, "repo version").unwrap();
        std::fs::write(&dest, "local version").unwrap();

        let link = ResolvedLink {
            src,
            dest,
            copy: false,
        };

        assert_eq!(link.check(), LinkResult::Conflict);
    }

    #[test]
    fn test_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("nested").join("deep").join("link.txt");

        std::fs::write(&src, "hello").unwrap();

        let link = ResolvedLink {
            src: src.clone(),
            dest: dest.clone(),
            copy: false,
        };

        let result = link.apply();
        assert_eq!(result, LinkResult::Linked);
        assert!(dest.exists());
    }

    #[test]
    fn test_copy_mode() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("copy.txt");

        std::fs::write(&src, "hello world").unwrap();

        let link = ResolvedLink {
            src: src.clone(),
            dest: dest.clone(),
            copy: true,
        };

        let result = link.apply();
        assert_eq!(result, LinkResult::Linked);
        assert!(!dest.symlink_metadata().unwrap().is_symlink());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello world");
    }

    #[test]
    fn test_dry_run_no_filesystem_changes() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("link.txt");

        std::fs::write(&src, "hello").unwrap();

        let link = ResolvedLink {
            src,
            dest: dest.clone(),
            copy: false,
        };

        let result = process_link(&link, true);
        assert_eq!(result, LinkResult::Linked);
        assert!(!dest.exists());
    }

    #[test]
    fn test_idempotent_apply() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("link.txt");

        std::fs::write(&src, "hello").unwrap();

        let link = ResolvedLink {
            src: src.clone(),
            dest: dest.clone(),
            copy: false,
        };

        // First apply
        let result1 = link.apply();
        assert_eq!(result1, LinkResult::Linked);

        // Second check should be AlreadyCorrect
        let result2 = process_link(&link, false);
        assert_eq!(result2, LinkResult::AlreadyCorrect);
    }

    #[test]
    fn test_missing_source_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("nonexistent.txt");
        let dest = dir.path().join("link.txt");

        let link = ResolvedLink {
            src,
            dest,
            copy: false,
        };

        let result = link.apply();
        assert!(matches!(result, LinkResult::Failed(msg) if msg.contains("source file not found")));
    }

    #[test]
    fn test_check_returns_ready_for_absent_dest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dest = dir.path().join("absent.txt");

        std::fs::write(&src, "hello").unwrap();

        let link = ResolvedLink {
            src,
            dest,
            copy: false,
        };

        assert_eq!(link.check(), LinkResult::Ready);
    }
}
