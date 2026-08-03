//! Visual constants and print helpers for consistent terminal output.
//!
//! All themed output (symbols, colours, the ASCII banner) is defined here
//! so the rest of the codebase stays free of formatting details.

use console::style;

/// Indentation prefix used across all output lines.
pub const INDENT: &str = "  ";

/// Success checkmark.
pub const SYMBOL_OK: &str = "\u{2713}";
/// Failure cross.
pub const SYMBOL_ERR: &str = "\u{2717}";
/// Warning triangle.
pub const SYMBOL_WARN: &str = "\u{26a0}";
/// Directional arrow for linking output.
pub const SYMBOL_ARROW: &str = "\u{2192}";

/// Prints a success message to stdout with green checkmark.
pub fn print_success(msg: &str) {
    println!("{INDENT}{} {msg}", style(SYMBOL_OK).green().bold());
}

/// Prints an error message to stderr with red cross.
pub fn print_error(msg: &str) {
    eprintln!("{INDENT}{} {msg}", style(SYMBOL_ERR).red().bold());
}

/// Prints a warning message to stderr with yellow triangle.
pub fn print_warn(msg: &str) {
    eprintln!("{INDENT}{} {msg}", style(SYMBOL_WARN).yellow().bold());
}

/// Prints the anvil banner with version info (shown on `init`).
///
/// Typographic wordmark only — no cryptic glyph art. Uses a small
/// FIGlet-style "slant" spelling of *anvil* so it reads clearly in
/// any monospace font.
///
/// ```text
///                _ __
///   ____ ____  _(_) /
///  / __ `/ __ \/ / /
/// / /_/ / / / / / /
/// \__,_/_/ /_/_/_/
///
///   forge your machine · v0.1.0
/// ```
pub fn print_header() {
    let version = env!("CARGO_PKG_VERSION");

    // Warm accent for the wordmark (works on dark terminals; bold helps on light)
    let word = |s: &str| style(s).color256(180).bold();

    println!();
    println!("{INDENT}{}", word(r"               _ __"));
    println!("{INDENT}{}", word(r"  ____ ____  _(_) /"));
    println!("{INDENT}{}", word(r" / __ `/ __ \/ / / "));
    println!("{INDENT}{}", word(r"/ /_/ / / / / / /  "));
    println!("{INDENT}{}", word(r"\__,_/_/ /_/_/_/   "));
    println!();
    println!(
        "{INDENT}{} {}",
        style("forge your machine").dim(),
        style(format!("· v{version}")).dim(),
    );
    println!();
}

#[cfg(test)]
mod tests {
    #[test]
    fn header_does_not_panic() {
        super::print_header();
    }
}
