//! Git backend for anvil.
//!
//! Abstracts git operations behind the [`GitBackend`] trait so the rest of
//! the codebase never shells out to git directly. The default [`ShellGit`]
//! implementation calls the system `git` binary; a future `libgit2` backend
//! can be swapped in without touching command code.

use std::path::Path;
use std::process::Command;

use crate::error::{AnvilError, Result};

/// Abstracts git operations. Implementations must be able to clone a repo.
pub trait GitBackend {
    /// Clones a repository from `url` into `dest`.
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()>;
}

/// Git backend that shells out to the system `git` binary.
pub struct ShellGit;

impl ShellGit {
    /// Checks that the `git` binary is available on `$PATH`.
    pub fn ensure_available() -> Result<()> {
        Command::new("git")
            .arg("--version")
            .output()
            .map_err(AnvilError::GitNotFound)?;
        Ok(())
    }
}

impl GitBackend for ShellGit {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["clone", "--depth=1", url])
            .arg(dest)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(AnvilError::GitNotFound)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            if detail.is_empty() {
                return Err(AnvilError::GitCloneFailed(url.to_string()));
            }
            return Err(AnvilError::GitCloneFailed(format!("{url}\n  {detail}")));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_git_available() {
        // Should succeed on any dev machine with git installed
        ShellGit::ensure_available().unwrap();
    }

    #[test]
    fn test_clone_local_repo() {
        let dir = tempdir().unwrap();

        // Create a source repo with a file
        let source = dir.path().join("source");
        std::fs::create_dir_all(&source).unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&source)
            .output()
            .unwrap();
        std::fs::write(source.join("hello.txt"), "world").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&source)
            .output()
            .unwrap();

        // Clone it via ShellGit
        let dest = dir.path().join("cloned");
        let git = ShellGit;
        git.clone_repo(source.to_str().unwrap(), &dest).unwrap();

        assert!(dest.join("hello.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dest.join("hello.txt")).unwrap(),
            "world"
        );
    }

    #[test]
    fn test_clone_invalid_url_fails() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("cloned");

        let git = ShellGit;
        let err = git
            .clone_repo("https://invalid.example.com/no-such-repo.git", &dest)
            .unwrap_err();

        assert!(matches!(err, AnvilError::GitCloneFailed(_)));
    }

    #[test]
    fn test_clone_uses_depth_one() {
        let dir = tempdir().unwrap();

        // Create a source repo with multiple commits
        let source = dir.path().join("source");
        std::fs::create_dir_all(&source).unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&source)
            .output()
            .unwrap();
        std::fs::write(source.join("a.txt"), "first").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "first"])
            .current_dir(&source)
            .output()
            .unwrap();
        std::fs::write(source.join("b.txt"), "second").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "second"])
            .current_dir(&source)
            .output()
            .unwrap();

        // Clone with ShellGit (uses --depth=1)
        // Use file:// protocol to force pack transport (local clones use
        // hardlinks and may ignore --depth)
        let dest = dir.path().join("cloned");
        let git = ShellGit;
        let url = format!("file://{}", source.display());
        git.clone_repo(&url, &dest).unwrap();

        // Verify shallow clone: only 1 commit in history
        let log = StdCommand::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(&dest)
            .output()
            .unwrap();
        let count: usize = String::from_utf8_lossy(&log.stdout).trim().parse().unwrap();
        assert_eq!(count, 1, "shallow clone should have exactly 1 commit");
    }
}
