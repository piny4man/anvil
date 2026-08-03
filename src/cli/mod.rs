pub mod add;
pub mod apply;
pub mod doctor;
pub mod init;
pub mod status;
pub mod sync;
pub mod undo;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anvil", about = "Dotfiles manager — forge your machine")]
#[command(
    long_about = "Interactive-by-default manager for personal machine config.\n\
\n\
Tracks files in any Git repo via anvil.toml, links them onto the system,\n\
optionally installs packages (Arch pacman/AUR), decrypts age secrets, and\n\
runs Linux hardening checks.\n\
\n\
Local state lives in ~/.config/anvil/ and ~/.local/state/anvil/.\n\
\n\
Safety: --yes accepts prompts without being destructive; --force is required\n\
to overwrite existing files (always after writing a backup journal)."
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Accept prompt defaults without asking (not destructive by itself)
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Print the plan only; change nothing on disk
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress non-error output (also auto-enabled when stdout is not a TTY)
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Allow overwriting conflicting files (backs up first; use with apply/add)
    #[arg(long, global = true)]
    pub force: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Clone a dots repo (or reuse checkout), scaffold anvil.toml if missing, apply
    Init {
        /// Git repository URL (HTTPS or SSH). Prompted if omitted.
        url: Option<String>,
        /// Profile(s) to activate; defaults from [machines], default_profile, or prompt
        #[arg(short, long)]
        profile: Vec<String>,
        /// Clone destination (default: ~/.dotfiles)
        #[arg(long)]
        dir: Option<String>,
    },
    /// git pull --rebase, then re-apply links
    Sync {
        /// Only pull; do not re-apply
        #[arg(long)]
        pull_only: bool,
    },
    /// Link files from the repo according to the active profile(s)
    Apply {
        /// Override active profile(s) for this run
        #[arg(short, long)]
        profile: Vec<String>,
        /// Install missing packages listed in the profile (pacman / AUR helpers)
        #[arg(long)]
        packages: bool,
        /// Run hardening checks; enforce sysctl if harden.mode = "enforce"
        #[arg(long)]
        harden: bool,
    },
    /// Move a file into the repo, link it back, and update anvil.toml
    Add {
        /// Path to an existing config file or directory on this machine
        file: PathBuf,
        /// Profile to attach the link to (prompted if omitted)
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Show link health, missing packages, and harden check results
    Status {
        #[arg(short, long)]
        profile: Vec<String>,
    },
    /// Diagnose git/manifest/symlinks and optional security posture
    Doctor,
    /// Restore paths from the latest backup journal under ~/.local/state/anvil/backups
    Undo,
}
