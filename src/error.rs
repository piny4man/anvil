use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, AnvilError>;

#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    #[error("failed to read {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config: {0}")]
    ConfigParse(String),

    #[error("git not found: {0}")]
    GitNotFound(#[source] std::io::Error),

    #[error("git clone failed for {0}")]
    GitCloneFailed(String),

    #[error("git pull failed: {0}")]
    GitPullFailed(String),

    #[error("git command failed: {0}")]
    GitFailed(String),

    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    #[error("profile cycle detected: {0}")]
    ProfileCycle(String),

    #[error("failed to create symlink at {path}: {source}")]
    SymlinkFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to copy to {path}: {source}")]
    CopyFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not determine home directory")]
    HomeDirNotFound,

    #[error("no anvil.toml found at {0}")]
    ManifestNotFound(PathBuf),

    #[error("dotfiles repo not configured; run `anvil init` first")]
    RepoNotConfigured,

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("prompt cancelled by user")]
    PromptCancelled,

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("hook failed: {0}")]
    HookFailed(String),

    #[error("package error: {0}")]
    Package(String),

    #[error("secrets error: {0}")]
    Secrets(String),

    #[error("harden error: {0}")]
    Harden(String),

    #[error("backup error: {0}")]
    Backup(String),

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{0}")]
    Other(String),
}

impl AnvilError {
    /// Process exit code: 0 unused, 1 error, 2 drift/conflicts.
    pub fn exit_code(&self) -> i32 {
        match self {
            AnvilError::Other(msg) if msg.contains("conflict") || msg.contains("drift") => 2,
            _ => 1,
        }
    }
}
