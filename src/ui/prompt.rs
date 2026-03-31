//! Interactive prompt wrappers built on [`inquire`].
//!
//! These functions are the low-level building blocks called by
//! [`super::UiContext`]. They should not be used directly by command
//! code -- go through `UiContext` so that `--yes` short-circuiting
//! is handled automatically.

use std::fmt::Display;
use std::path::Path;

use inquire::{Confirm, InquireError, MultiSelect, Select, Text};

use crate::error::{AnvilError, Result};

/// Maps `InquireError` to `AnvilError`, treating user cancellation and
/// interrupt (Ctrl-C) as `PromptCancelled`.
fn map_inquire_err(e: InquireError) -> AnvilError {
    match e {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            AnvilError::PromptCancelled
        }
        other => AnvilError::Other(other.to_string()),
    }
}

/// Prompts for free-form text input with an optional default value.
pub fn text(msg: &str, default: Option<&str>) -> Result<String> {
    let mut prompt = Text::new(msg);
    if let Some(d) = default {
        prompt = prompt.with_default(d);
    }
    prompt.prompt().map_err(map_inquire_err)
}

/// Prompts for a yes/no confirmation.
pub fn confirm(msg: &str, default: bool) -> Result<bool> {
    Confirm::new(msg)
        .with_default(default)
        .prompt()
        .map_err(map_inquire_err)
}

/// Prompts the user to pick one option. Returns the chosen index.
pub fn select<T: Display>(msg: &str, options: Vec<T>, default: usize) -> Result<usize> {
    Select::new(msg, options)
        .with_starting_cursor(default)
        .raw_prompt()
        .map(|opt| opt.index)
        .map_err(map_inquire_err)
}

/// Prompts the user to pick multiple options. Returns the chosen indices.
pub fn multi_select<T: Display>(msg: &str, options: Vec<T>) -> Result<Vec<usize>> {
    MultiSelect::new(msg, options)
        .raw_prompt()
        .map(|opts| opts.into_iter().map(|o| o.index).collect())
        .map_err(map_inquire_err)
}

/// Action the user can take when a target file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictAction {
    /// Replace the existing file with the new symlink/copy.
    Overwrite,
    /// Leave the existing file untouched.
    Skip,
    /// Display a diff between the existing file and the source.
    ShowDiff,
}

/// Asks the user how to handle a conflicting file at `path`.
pub fn conflict_resolution(path: &Path) -> Result<ConflictAction> {
    let options = vec!["Overwrite", "Skip", "Show diff"];
    let idx = Select::new(
        &format!("{} already exists. What to do?", path.display()),
        options,
    )
    .raw_prompt()
    .map(|opt| opt.index)
    .map_err(map_inquire_err)?;

    Ok(match idx {
        0 => ConflictAction::Overwrite,
        1 => ConflictAction::Skip,
        2 => ConflictAction::ShowDiff,
        _ => unreachable!("select returned index outside option bounds"),
    })
}
