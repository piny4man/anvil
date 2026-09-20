use clap::Parser;
use console::Term;

use anvil::cli::{self, Cli, Command};
use anvil::ui::UiContext;

fn main() {
    let cli = Cli::parse();

    let quiet = cli.quiet || !Term::stdout().is_term();
    let ctx = UiContext::new(cli.yes, quiet, cli.dry_run).with_force(cli.force);

    let result = match cli.command.unwrap_or(Command::Status {
        profile: Vec::new(),
    }) {
        Command::Init {
            url,
            profile,
            dir,
            no_apply,
        } => cli::init::run(url, profile, dir, no_apply, &ctx),
        Command::Sync { pull_only } => cli::sync::run(pull_only, &ctx),
        Command::Apply {
            profile,
            packages,
            harden,
            aur_helper,
        } => cli::apply::run(profile, packages, harden, aur_helper, &ctx),
        Command::Add { file, profile } => cli::add::run(file, profile, &ctx),
        Command::Status { profile } => cli::status::run(profile, &ctx),
        Command::Doctor => cli::doctor::run(&ctx),
        Command::Undo { list, id } => cli::undo::run(list, id, &ctx),
    };

    if let Err(e) = result {
        ctx.error(&e.to_string());
        std::process::exit(e.exit_code());
    }
}
