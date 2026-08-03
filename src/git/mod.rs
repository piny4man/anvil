pub mod backend;
pub mod shell;

pub use backend::{GitBackend, PullResult};
pub use shell::ShellGit;
