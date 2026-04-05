//! Terminal I/O layer for anvil.
//!
//! All user-facing output and interactive prompts flow through this module.
//! Commands receive a [`UiContext`] by reference and call its methods instead
//! of using `println!` or `inquire` directly. This keeps flag handling
//! (`--yes`, `--quiet`, `--dry-run`) in one place.

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
/// Encapsulates `--yes`, `--quiet`, and `--dry-run` flags so commands
/// do not sprinkle conditionals.
pub struct UiContext {
    pub yes: bool,
    pub quiet: bool,
    pub dry_run: bool,
}

impl UiContext {
    /// Creates a new `UiContext` from the global CLI flags.
    pub fn new(yes: bool, quiet: bool, dry_run: bool) -> Self {
        Self {
            yes,
            quiet,
            dry_run,
        }
    }

    // -- Prompt methods (short-circuit on --yes) --

    /// Prompts for free-form text. Under `--yes`, returns `default` or
    /// errors with `PromptCancelled` if no default is provided.
    pub fn text(&self, msg: &str, default: Option<&str>) -> Result<String> {
        if self.yes {
            return default
                .map(|d| d.to_string())
                .ok_or(AnvilError::PromptCancelled);
        }
        prompt::text(msg, default)
    }

    /// Prompts for yes/no confirmation. Under `--yes`, returns `default`.
    pub fn confirm(&self, msg: &str, default: bool) -> Result<bool> {
        if self.yes {
            return Ok(default);
        }
        prompt::confirm(msg, default)
    }

    /// Prompts the user to pick one option. Returns the chosen index.
    /// Under `--yes`, returns `default` (validated against `options.len()`).
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

    /// Prompts for multiple selections. Under `--yes`, selects all options.
    pub fn multi_select<T: Display>(&self, msg: &str, options: Vec<T>) -> Result<Vec<usize>> {
        if self.yes {
            return Ok((0..options.len()).collect());
        }
        prompt::multi_select(msg, options)
    }

    /// Asks how to handle a conflicting file. Under `--yes`, returns `Skip`
    /// (safe, non-destructive default).
    pub fn conflict_resolution(&self, path: &Path) -> Result<ConflictAction> {
        if self.yes {
            return Ok(ConflictAction::Skip);
        }
        prompt::conflict_resolution(path)
    }

    // -- Output methods (respect --quiet) --

    /// Starts an animated spinner. Returns `None` when `--quiet` is active.
    pub fn spinner(&self, msg: &str) -> Option<Spinner> {
        if self.quiet {
            return None;
        }
        Some(spinner::start(msg))
    }

    /// Prints a green success message. Suppressed by `--quiet`.
    pub fn success(&self, msg: &str) {
        if !self.quiet {
            theme::print_success(msg);
        }
    }

    /// Prints a yellow warning to stderr. Always shown regardless of `--quiet`.
    pub fn warn(&self, msg: &str) {
        theme::print_warn(msg);
    }

    /// Prints a red error to stderr. Always shown regardless of `--quiet`.
    pub fn error(&self, msg: &str) {
        theme::print_error(msg);
    }

    /// Prints the anvil ASCII banner. Suppressed by `--quiet`.
    pub fn header(&self) {
        if !self.quiet {
            theme::print_header();
        }
    }
}
