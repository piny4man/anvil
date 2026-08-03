//! Shell hook execution with live streamed output.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use crate::error::{AnvilError, Result};
use crate::ui::UiContext;

/// Run a list of hook scripts relative to `repo`. Streams output with `│ ` prefix.
pub fn run_hooks(repo: &Path, hooks: &[String], ctx: &UiContext) -> Result<()> {
    for hook in hooks {
        run_one(repo, hook, ctx)?;
    }
    Ok(())
}

fn run_one(repo: &Path, hook: &str, ctx: &UiContext) -> Result<()> {
    // Security: only allow relative paths inside the repo.
    let path = Path::new(hook);
    if path.is_absolute() {
        return Err(AnvilError::HookFailed(format!(
            "hook must be repo-relative (got absolute path `{hook}`)"
        )));
    }
    if hook.contains("..") {
        return Err(AnvilError::HookFailed(format!(
            "hook path must not contain `..` (`{hook}`)"
        )));
    }

    let full = repo.join(hook);
    if !full.exists() {
        return Err(AnvilError::HookFailed(format!(
            "hook not found: {}",
            full.display()
        )));
    }

    if ctx.dry_run {
        ctx.success(&format!("would run hook: {hook}"));
        return Ok(());
    }

    ctx.success(&format!("Running hook: {hook}"));

    let mut child = Command::new("bash")
        .arg(&full)
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AnvilError::HookFailed(format!("failed to spawn `{hook}`: {e}")))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let quiet = ctx.quiet;
    let out_handle = stdout.map(|out| {
        thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(std::result::Result::ok) {
                if !quiet {
                    println!("  │ {line}");
                }
            }
        })
    });
    let err_handle = stderr.map(|err| {
        thread::spawn(move || {
            let reader = BufReader::new(err);
            for line in reader.lines().map_while(std::result::Result::ok) {
                if !quiet {
                    eprintln!("  │ {line}");
                }
            }
        })
    });

    if let Some(h) = out_handle {
        let _ = h.join();
    }
    if let Some(h) = err_handle {
        let _ = h.join();
    }

    let status = child
        .wait()
        .map_err(|e| AnvilError::HookFailed(e.to_string()))?;
    if !status.success() {
        return Err(AnvilError::HookFailed(format!(
            "`{hook}` exited with {status}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn rejects_absolute() {
        let dir = tempdir().unwrap();
        let ctx = UiContext::new(true, true, true);
        let err = run_hooks(dir.path(), &["/bin/true".into()], &ctx).unwrap_err();
        assert!(matches!(err, AnvilError::HookFailed(_)));
    }

    #[test]
    fn rejects_dotdot() {
        let dir = tempdir().unwrap();
        let ctx = UiContext::new(true, true, true);
        let err = run_hooks(dir.path(), &["../evil.sh".into()], &ctx).unwrap_err();
        assert!(matches!(err, AnvilError::HookFailed(_)));
    }

    #[test]
    fn dry_run_skips_execution() {
        let dir = tempdir().unwrap();
        let ctx = UiContext::new(true, true, true);
        // missing file is ok in dry_run? we check exists first — need file
        fs::write(dir.path().join("h.sh"), "#!/bin/bash\nexit 1\n").unwrap();
        run_hooks(dir.path(), &["h.sh".into()], &ctx).unwrap();
    }
}
