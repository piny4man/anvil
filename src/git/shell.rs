use std::path::Path;
use std::process::{Command, Stdio};

use super::backend::{GitBackend, PullResult};
use crate::error::{AnvilError, Result};

/// Default backend: shells out to the `git` binary.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShellGit;

impl ShellGit {
    pub fn new() -> Self {
        Self
    }

    fn run(args: &[&str], cwd: Option<&Path>) -> Result<std::process::Output> {
        let mut cmd = Command::new("git");
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.output().map_err(AnvilError::GitNotFound)
    }

    fn run_checked(args: &[&str], cwd: Option<&Path>) -> Result<String> {
        let output = Self::run(args, cwd)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let msg = if !stderr.is_empty() { stderr } else { stdout };
            return Err(AnvilError::GitFailed(msg));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

impl GitBackend for ShellGit {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()> {
        if dest.exists() {
            return Err(AnvilError::GitCloneFailed(format!(
                "destination already exists: {}",
                dest.display()
            )));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AnvilError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let dest_str = dest
            .to_str()
            .ok_or_else(|| AnvilError::InvalidPath(dest.display().to_string()))?;

        let output = Self::run(&["clone", url, dest_str], None)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(AnvilError::GitCloneFailed(if stderr.is_empty() {
                url.to_string()
            } else {
                format!("{url}: {stderr}")
            }));
        }
        Ok(())
    }

    fn pull(&self, repo_dir: &Path) -> Result<PullResult> {
        let output = Self::run(&["pull", "--rebase", "--autostash"], Some(repo_dir))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(AnvilError::GitPullFailed(stderr));
        }
        let summary = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let summary = if summary.is_empty() {
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        } else {
            summary
        };
        let was_updated = !summary.to_lowercase().contains("already up to date");
        Ok(PullResult {
            was_updated,
            summary: if summary.is_empty() {
                "pulled".into()
            } else {
                summary
            },
        })
    }

    fn status_clean(&self, repo_dir: &Path) -> Result<bool> {
        let out = Self::run_checked(&["status", "--porcelain"], Some(repo_dir))?;
        Ok(out.is_empty())
    }

    fn remote_url(&self, repo_dir: &Path) -> Result<Option<String>> {
        match Self::run_checked(&["remote", "get-url", "origin"], Some(repo_dir)) {
            Ok(url) if !url.is_empty() => Ok(Some(url)),
            Ok(_) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    fn is_repo(&self, dir: &Path) -> bool {
        dir.join(".git").exists()
            || Self::run(&["rev-parse", "--git-dir"], Some(dir))
                .map(|o| o.status.success())
                .unwrap_or(false)
    }
}
