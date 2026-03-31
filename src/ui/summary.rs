//! Post-apply summary reporting.
//!
//! [`ApplySummary`] accumulates per-file outcomes during `anvil apply` and
//! prints a coloured one-line summary when the operation finishes.

use console::style;

use super::UiContext;
use super::theme::{INDENT, SYMBOL_ERR, SYMBOL_OK, SYMBOL_WARN};

/// Tracks the outcome counts for a batch apply operation.
///
/// Increment `linked`, `skipped`, and `failed` as each file is processed,
/// then call [`print`](ApplySummary::print) to render the final summary.
#[derive(Debug, Default)]
pub struct ApplySummary {
    /// Number of files successfully linked.
    pub linked: usize,
    /// Number of files skipped (already correct or user chose to skip).
    pub skipped: usize,
    /// Number of files that failed to link.
    pub failed: usize,
}

impl ApplySummary {
    /// Creates a new summary with all counters at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Prints the summary line. Respects `--quiet` via the provided context.
    /// The leading symbol reflects the worst outcome: red cross if any failed,
    /// yellow warning if any skipped, green checkmark otherwise.
    pub fn print(&self, ctx: &UiContext) {
        if ctx.quiet {
            return;
        }

        let mut parts = Vec::new();

        if self.linked > 0 {
            parts.push(format!("{} linked", style(self.linked).green().bold()));
        }
        if self.skipped > 0 {
            parts.push(format!("{} skipped", style(self.skipped).yellow().bold()));
        }
        if self.failed > 0 {
            parts.push(format!("{} failed", style(self.failed).red().bold()));
        }

        let joined = parts.join(", ");

        if self.failed > 0 {
            println!(
                "\n{INDENT}{} Failed. {joined}",
                style(SYMBOL_ERR).red().bold()
            );
        } else if self.skipped > 0 {
            println!(
                "\n{INDENT}{} Done! {joined}",
                style(SYMBOL_WARN).yellow().bold()
            );
        } else {
            println!(
                "\n{INDENT}{} Done! {joined}",
                style(SYMBOL_OK).green().bold()
            );
        }
    }
}
