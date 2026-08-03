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

/// Banner lines spelling "anvil" (dense block style).
const BANNER: &[&str] = &[
    r"                                   ███  ████ ",
    r"                                  ▒▒▒  ▒▒███ ",
    r"  ██████   ████████   █████ █████ ████  ▒███ ",
    r" ▒▒▒▒▒███ ▒▒███▒▒███ ▒▒███ ▒▒███ ▒▒███  ▒███ ",
    r"  ███████  ▒███ ▒███  ▒███  ▒███  ▒███  ▒███ ",
    r" ███▒▒███  ▒███ ▒███  ▒▒███ ███   ▒███  ▒███ ",
    r"▒▒████████ ████ █████  ▒▒█████    █████ █████",
    r" ▒▒▒▒▒▒▒▒ ▒▒▒▒ ▒▒▒▒▒    ▒▒▒▒▒    ▒▒▒▒▒ ▒▒▒▒▒ ",
];

/// Prints the anvil banner with version info (shown on `init`).
pub fn print_header() {
    let version = env!("CARGO_PKG_VERSION");

    println!();
    for line in BANNER {
        // Full blocks warm amber; light blocks slightly dimmer steel
        println!("{INDENT}{}", colorize_banner_line(line));
    }
    println!(
        "{INDENT}{} {}",
        style("forge your machine").dim(),
        style(format!("· v{version}")).dim(),
    );
    println!();
}

/// Colour full `█` blocks amber and light `▒` blocks cooler/dimmer for depth.
fn colorize_banner_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len() * 8);
    for ch in line.chars() {
        match ch {
            '█' => {
                out.push_str(&style("█").color256(180).bold().to_string());
            }
            '▒' => {
                out.push_str(&style("▒").color256(73).to_string());
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn header_does_not_panic() {
        super::print_header();
    }
}
