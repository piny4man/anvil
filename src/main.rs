use clap::Parser;
use console::Term;

use anvil::cli::{self, Cli, Command};
use anvil::ui::UiContext;

fn main() {
    let cli = Cli::parse();

    let quiet = cli.quiet || !Term::stdout().is_term();
    let ctx = UiContext::new(cli.yes, quiet, cli.dry_run);

    let result = match cli.command.unwrap_or(Command::Status) {
        Command::Init { url, profile } => cli::init::run(url, profile, &ctx),
        Command::Sync => cli::sync::run(&ctx),
        Command::Apply { profile } => cli::apply::run(profile, &ctx),
        Command::Add { file, profile } => cli::add::run(file, profile, &ctx),
        Command::Status => cli::status::run(&ctx),
        Command::Doctor => cli::doctor::run(&ctx),
    };

    if let Err(e) = result {
        ctx.error(&e.to_string());
        std::process::exit(1);
    }
}
