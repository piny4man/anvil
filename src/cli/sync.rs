use crate::config::{LocalConfig, discover_repo};
use crate::error::Result;
use crate::git::{GitBackend, ShellGit};
use crate::ui::UiContext;

pub fn run(pull_only: bool, ctx: &UiContext) -> Result<()> {
    let local = LocalConfig::load()?;
    let repo = discover_repo(&local)?;
    let git = ShellGit::new();

    if !git.is_repo(&repo) {
        ctx.warn(&format!(
            "{} is not a git repository; skipping pull",
            repo.display()
        ));
    } else if ctx.dry_run {
        ctx.success(&format!("would pull in {}", repo.display()));
    } else {
        let spinner = ctx.spinner("Pulling latest changes...");
        match git.pull(&repo) {
            Ok(result) => {
                if let Some(s) = spinner {
                    s.success(&result.summary);
                } else {
                    ctx.success(&result.summary);
                }
            }
            Err(e) => {
                if let Some(s) = spinner {
                    s.fail("Pull failed");
                }
                return Err(e);
            }
        }
    }

    if pull_only {
        return Ok(());
    }

    ctx.info("Re-applying links...");
    crate::cli::apply::run(local.profiles.clone(), false, false, None, ctx)
}
