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

    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    #[error("failed to create symlink at {path}: {source}")]
    SymlinkFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not determine home directory")]
    HomeDirNotFound,

    #[error("no anvil.toml found at {0}")]
    ManifestNotFound(PathBuf),

    #[error("prompt cancelled by user")]
    PromptCancelled,

    #[error("{0}")]
    Other(String),
}
