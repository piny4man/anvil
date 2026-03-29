pub mod add;
pub mod apply;
pub mod doctor;
pub mod init;
pub mod status;
pub mod sync;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anvil", about = "Dotfiles manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Accept all defaults, skip prompts
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Show what would happen without making changes
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Bootstrap dotfiles on a new machine
    Init {
        url: Option<String>,
        #[arg(short, long)]
        profile: Vec<String>,
    },
    /// Pull latest changes and re-apply
    Sync,
    /// Apply dotfiles to the system
    Apply {
        #[arg(short, long)]
        profile: Vec<String>,
    },
    /// Adopt an existing file into the dotfiles repo
    Add {
        file: PathBuf,
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Show current link status
    Status,
    /// Check for common issues
    Doctor,
}
