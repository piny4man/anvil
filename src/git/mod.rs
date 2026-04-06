//! Git backend for anvil.
//!
//! Abstracts git operations behind the [`GitBackend`] trait so the rest of
//! the codebase never shells out to git directly. The default [`ShellGit`]
//! implementation calls the system `git` binary; a future `libgit2` backend
//! can be swapped in without touching command code.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{AnvilError, Result};

/// Result of a `git pull` operation.
#[derive(Debug)]
pub struct PullResult {
    /// Whether new changes were fetched from the remote.
    pub was_updated: bool,
    /// Human-readable summary. When changes were pulled this contains the
    /// diffstat line (e.g. "2 files changed, 3 insertions(+)"); when the repo
    /// is already up to date it is `"Already up to date"`.
    pub summary: String,
}

/// Abstracts git operations so command code never shells out directly.
pub trait GitBackend {
    /// Clones a repository from `url` into `dest`.
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()>;

    /// Pulls the latest changes in `repo_dir` from its configured remote.
    fn pull(&self, repo_dir: &Path) -> Result<PullResult>;
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

    fn pull(&self, repo_dir: &Path) -> Result<PullResult> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_dir)
            .args(["pull", "--ff-only"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(AnvilError::GitNotFound)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            if detail.is_empty() {
                return Err(AnvilError::GitPullFailed("unknown error".to_string()));
            }
            return Err(AnvilError::GitPullFailed(detail.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // TODO: This is locale-dependent. A more robust approach would be to
        // compare HEAD before and after the pull.
        let was_updated = !stdout.contains("Already up");

        let summary = if was_updated {
            // Extract the stat summary line (e.g. "2 files changed, 3 insertions(+)")
            stdout
                .lines()
                .rev()
                .find(|line| line.contains("changed"))
                .unwrap_or("Changes pulled")
                .trim()
                .to_string()
        } else {
            "Already up to date".to_string()
        };

        Ok(PullResult {
            was_updated,
            summary,
        })
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

    /// Helper: creates a git repo at `dir` with user config and an initial commit.
    fn init_repo(dir: &std::path::Path) {
        std::fs::create_dir_all(dir).unwrap();
        StdCommand::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[test]
    fn test_pull_no_changes() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        init_repo(&source);

        std::fs::write(source.join("file.txt"), "hello").unwrap();
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

        // Clone it
        let clone = dir.path().join("clone");
        StdCommand::new("git")
            .args(["clone"])
            .arg(&source)
            .arg(&clone)
            .output()
            .unwrap();

        // Pull with no upstream changes
        let git = ShellGit;
        let result = git.pull(&clone).unwrap();
        assert!(!result.was_updated);
        assert_eq!(result.summary, "Already up to date");
    }

    #[test]
    fn test_pull_with_changes() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        init_repo(&source);

        std::fs::write(source.join("file.txt"), "hello").unwrap();
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

        // Clone it
        let clone = dir.path().join("clone");
        StdCommand::new("git")
            .args(["clone"])
            .arg(&source)
            .arg(&clone)
            .output()
            .unwrap();

        // Add a new commit to source
        std::fs::write(source.join("new.txt"), "new content").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&source)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "add new file"])
            .current_dir(&source)
            .output()
            .unwrap();

        // Pull — should detect changes
        let git = ShellGit;
        let result = git.pull(&clone).unwrap();
        assert!(result.was_updated);
        assert!(
            result.summary.contains("changed"),
            "expected 'changed' in summary: {}",
            result.summary
        );
        assert!(clone.join("new.txt").exists());
    }

    #[test]
    fn test_pull_not_a_repo_fails() {
        let dir = tempdir().unwrap();
        let not_repo = dir.path().join("not-a-repo");
        std::fs::create_dir(&not_repo).unwrap();

        let git = ShellGit;
        let err = git.pull(&not_repo).unwrap_err();
        assert!(
            matches!(err, AnvilError::GitPullFailed(_)),
            "expected GitPullFailed, got: {err}"
        );
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
