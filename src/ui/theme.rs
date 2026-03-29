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

/// Prints the anvil ASCII banner with version info.
pub fn print_header() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("{INDENT}{}", style("▗▄▖ █▄ █ █ █ ██▄ █").bold());
    println!(" {}", style("▐▌ ▐▌█ ▀█ ▀▄▀ █▄█ █▄▄").bold());
    println!(
        " {} {}",
        style("dotfiles manager").dim(),
        style(format!("v{version}")).dim(),
    );
    println!();
}
