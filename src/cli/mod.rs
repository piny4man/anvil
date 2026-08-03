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
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Accept all defaults, skip prompts (not destructive by itself)
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Show what would happen without making changes
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Allow destructive overwrites (always backs up first)
    #[arg(long, global = true)]
    pub force: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Bootstrap dotfiles on a new machine
    Init {
        url: Option<String>,
        #[arg(short, long)]
        profile: Vec<String>,
        /// Directory to clone into (default: ~/.dotfiles)
        #[arg(long)]
        dir: Option<String>,
    },
    /// Pull latest changes and re-apply
    Sync {
        /// Skip re-apply after pull
        #[arg(long)]
        pull_only: bool,
    },
    /// Apply dotfiles (and optional packages/harden) to the system
    Apply {
        #[arg(short, long)]
        profile: Vec<String>,
        /// Also install missing packages from the manifest
        #[arg(long)]
        packages: bool,
        /// Run hardening checks (and enforce if mode=enforce)
        #[arg(long)]
        harden: bool,
    },
    /// Adopt an existing file into the dotfiles repo
    Add {
        file: PathBuf,
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Show current link / package / harden status
    Status {
        #[arg(short, long)]
        profile: Vec<String>,
    },
    /// Check for setup issues and security posture
    Doctor,
    /// Restore files from the latest backup journal
    Undo,
}
