pub mod local;
pub mod manifest;

pub use local::{LocalConfig, discover_repo, expand_tilde};
pub use manifest::{AnvilMeta, Hooks, Link, Manifest, Profile};
