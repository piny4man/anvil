//! Terminal I/O layer for anvil.
//!
//! All user-facing output and interactive prompts flow through this module.
//! Commands receive a [`UiContext`] by reference and call its methods instead
//! of using `println!` or `inquire` directly. This keeps flag handling
//! (`--yes`, `--quiet`, `--dry-run`, `--force`) in one place.
//!
//! ## Non-interactive defaults (`--yes`)
//!
//! | Prompt | `--yes` behavior |
//! |--------|------------------|
//! | text | returns provided default, else error |
//! | confirm | returns the caller's default |
//! | select | returns `default` index |
//! | multi_select | returns **empty** (caller must supply defaults) |
//! | conflict | **Skip** unless `--force` → Overwrite |
//!
//! `--yes` accepts prompts; `--force` allows destructive overwrites.

pub mod prompt;
pub mod spinner;
pub mod summary;
pub mod theme;

use std::fmt::Display;
use std::path::Path;

use crate::error::{AnvilError, Result};

use self::prompt::ConflictAction;
use self::spinner::Spinner;

/// Central UI controller passed to every command by reference.
pub struct UiContext {
    pub yes: bool,
    pub quiet: bool,
    pub dry_run: bool,
    /// Allow destructive overwrites (with backup). Distinct from `--yes`.
    pub force: bool,
}

impl UiContext {
    pub fn new(yes: bool, quiet: bool, dry_run: bool) -> Self {
        Self {
            yes,
            quiet,
            dry_run,
            force: false,
        }
    }

    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    // -- Prompt methods (short-circuit on --yes) --

    pub fn text(&self, msg: &str, default: Option<&str>) -> Result<String> {
        if self.yes {
            return default
                .map(|d| d.to_string())
                .ok_or(AnvilError::PromptCancelled);
        }
        prompt::text(msg, default)
    }

    pub fn confirm(&self, msg: &str, default: bool) -> Result<bool> {
        if self.yes {
            return Ok(default);
        }
        prompt::confirm(msg, default)
    }

    pub fn select<T: Display>(&self, msg: &str, options: Vec<T>, default: usize) -> Result<usize> {
        if self.yes {
            return if default < options.len() {
                Ok(default)
            } else {
                Err(AnvilError::Other(format!(
                    "select default index {default} out of range (len {})",
                    options.len()
                )))
            };
        }
        prompt::select(msg, options, default)
    }

    /// Under `--yes`, returns **no** selections. Callers should pass explicit
    /// defaults (default_profile / machines) instead of selecting all.
    pub fn multi_select<T: Display>(&self, msg: &str, options: Vec<T>) -> Result<Vec<usize>> {
        if self.yes {
            return Ok(Vec::new());
        }
        prompt::multi_select(msg, options)
    }

    /// Conflict handling: `--force` → Overwrite; `--yes` without force → Skip;
    /// interactive → prompt.
    pub fn conflict_resolution(&self, path: &Path) -> Result<ConflictAction> {
        if self.force {
            return Ok(ConflictAction::Overwrite);
        }
        if self.yes {
            return Ok(ConflictAction::Skip);
        }
        prompt::conflict_resolution(path)
    }

    // -- Output methods --

    pub fn spinner(&self, msg: &str) -> Option<Spinner> {
        if self.quiet {
            return None;
        }
        Some(spinner::start(msg))
    }

    pub fn success(&self, msg: &str) {
        if !self.quiet {
            theme::print_success(msg);
        }
    }

    pub fn warn(&self, msg: &str) {
        theme::print_warn(msg);
    }

    pub fn error(&self, msg: &str) {
        theme::print_error(msg);
    }

    pub fn header(&self) {
        if !self.quiet {
            theme::print_header();
        }
    }

    pub fn info(&self, msg: &str) {
        if !self.quiet {
            println!("  {msg}");
        }
    }

    /// Always print. Used for `--list` / piped output that must remain visible.
    pub fn line(&self, msg: &str) {
        println!("  {msg}");
    }
}
