use std::path::Path;

use crate::error::Result;

/// Result of a `git pull`.
#[derive(Debug, Clone)]
pub struct PullResult {
    pub was_updated: bool,
    pub summary: String,
}

/// Git operations used by anvil commands.
pub trait GitBackend {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()>;
    fn pull(&self, repo_dir: &Path) -> Result<PullResult>;
    fn status_clean(&self, repo_dir: &Path) -> Result<bool>;
    fn remote_url(&self, repo_dir: &Path) -> Result<Option<String>>;
    fn is_repo(&self, dir: &Path) -> bool;
}
