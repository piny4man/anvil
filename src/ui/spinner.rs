//! Animated terminal spinner for long-running operations.
//!
//! Wraps [`indicatif::ProgressBar`] in a thin [`Spinner`] type that
//! exposes only three terminal states: [`Spinner::success`], [`Spinner::warn`],
//! and [`Spinner::fail`]. Create one via [`start`] (or, preferably,
//! through [`super::UiContext::spinner`] which respects `--quiet`).

use std::time::Duration;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use super::theme::{SYMBOL_ERR, SYMBOL_OK, SYMBOL_WARN};

/// A running terminal spinner. Consuming methods stop the animation and
/// print a final status line.
pub struct Spinner(ProgressBar);

/// Creates and immediately starts a braille-dot spinner with the given message.
pub fn start(msg: &str) -> Spinner {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("  {spinner:.cyan} {msg}")
            .expect("hardcoded spinner template is valid"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    Spinner(pb)
}

impl Spinner {
    /// Stops the spinner and prints a green checkmark with the given message.
    pub fn success(self, msg: &str) {
        self.0
            .finish_with_message(format!("{} {msg}", style(SYMBOL_OK).green().bold()));
    }

    /// Stops the spinner and prints a yellow warning symbol with the given message.
    pub fn warn(self, msg: &str) {
        self.0
            .finish_with_message(format!("{} {msg}", style(SYMBOL_WARN).yellow().bold()));
    }

    /// Stops the spinner and prints a red cross with the given message.
    pub fn fail(self, msg: &str) {
        self.0
            .finish_with_message(format!("{} {msg}", style(SYMBOL_ERR).red().bold()));
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.0.is_finished() {
            self.0.finish_and_clear();
        }
    }
}
