use clap::Parser;

use anvil::cli::{self, Cli, Command};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command.unwrap_or(Command::Status) {
        Command::Init { url, profile } => cli::init::run(url, profile),
        Command::Sync => cli::sync::run(),
        Command::Apply { profile } => cli::apply::run(profile),
        Command::Add { file, profile } => cli::add::run(file, profile),
        Command::Status => cli::status::run(),
        Command::Doctor => cli::doctor::run(),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
