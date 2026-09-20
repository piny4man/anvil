use crate::error::{AnvilError, Result};
use crate::linker::BackupJournal;
use crate::ui::UiContext;

pub fn run(list: bool, id: Option<String>, ctx: &UiContext) -> Result<()> {
    if list {
        return list_journals(ctx);
    }

    let mut journal = if let Some(id) = id {
        BackupJournal::load_id(&id)?
            .ok_or_else(|| AnvilError::Backup(format!("no backup journal `{id}`")))?
    } else {
        BackupJournal::load_latest()?
            .ok_or_else(|| AnvilError::Backup("no backup journal found".into()))?
    };

    ctx.info(&format!(
        "Backup journal: {} ({} entries, {})",
        journal.id,
        journal.entries.len(),
        journal.status
    ));

    if ctx.dry_run {
        for e in &journal.entries {
            ctx.info(&format!("  would restore {} ({})", e.original_dest, e.kind));
        }
        ctx.success("Dry-run complete");
        return Ok(());
    }

    if !journal.is_active() {
        ctx.warn("This journal is already restored; skipping");
        return Ok(());
    }

    let ok = ctx.confirm("Restore files from this journal?", true)?;
    if !ok {
        ctx.warn("Undo cancelled");
        return Ok(());
    }

    let count = journal.restore()?;
    ctx.success(&format!("Restored {count} path(s)"));
    crate::packages::uninstall_recorded(&journal, ctx)?;
    Ok(())
}

fn list_journals(ctx: &UiContext) -> Result<()> {
    let journals = BackupJournal::list()?;
    if journals.is_empty() {
        ctx.warn("No backup journals found");
        return Ok(());
    }
    for j in &journals {
        ctx.line(&format!(
            "{}  {}  {} entries  {}",
            j.id,
            j.status,
            j.entries.len(),
            j.created_unix
        ));
    }
    Ok(())
}
