use crate::error::{AnvilError, Result};
use crate::linker::BackupJournal;
use crate::ui::UiContext;

pub fn run(ctx: &UiContext) -> Result<()> {
    let journal = BackupJournal::load_latest()?
        .ok_or_else(|| AnvilError::Backup("no backup journal found".into()))?;

    ctx.info(&format!(
        "Latest backup journal: {} ({} entries)",
        journal.id,
        journal.entries.len()
    ));

    if ctx.dry_run {
        for e in &journal.entries {
            ctx.info(&format!("  would restore {} ({})", e.original_dest, e.kind));
        }
        ctx.success("Dry-run complete");
        return Ok(());
    }

    let ok = ctx.confirm("Restore files from this journal?", true)?;
    if !ok {
        ctx.warn("Undo cancelled");
        return Ok(());
    }

    let count = journal.restore()?;
    ctx.success(&format!("Restored {count} path(s)"));
    Ok(())
}
