//! Backup journal for undo support.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{copy_file, remove_path};
use crate::error::{AnvilError, Result};
use crate::paths::backups_dir;

static JOURNAL_SEQ: AtomicU64 = AtomicU64::new(0);

const STATUS_ACTIVE: &str = "active";
const STATUS_RESTORED: &str = "restored";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub original_dest: String,
    pub backup_path: String,
    /// "file" | "symlink" | "dir" | "missing" | "sysctl-dropin" | "package"
    pub kind: String,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJournal {
    pub id: String,
    pub created_unix: u64,
    pub entries: Vec<JournalEntry>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(skip)]
    root: PathBuf,
}

fn default_status() -> String {
    STATUS_ACTIVE.to_string()
}

fn new_id() -> (String, u64) {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seq = JOURNAL_SEQ.fetch_add(1, Ordering::Relaxed);
    let id = format!("{}-{}", dur.as_nanos(), seq);
    (id, dur.as_secs())
}

impl BackupJournal {
    pub fn create() -> Result<Self> {
        Self::create_in(&backups_dir()?)
    }

    /// Create journal under a custom root (tests).
    pub fn create_in(base: &Path) -> Result<Self> {
        let (id, created_unix) = new_id();
        let root = base.join(&id);
        fs::create_dir_all(&root).map_err(|e| AnvilError::Backup(e.to_string()))?;
        Ok(Self {
            id,
            created_unix,
            entries: Vec::new(),
            status: STATUS_ACTIVE.into(),
            root,
        })
    }

    pub fn is_active(&self) -> bool {
        self.status.is_empty() || self.status == STATUS_ACTIVE
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Backup existing path before overwrite. Persists `journal.json` immediately.
    pub fn backup_path(&mut self, dest: &Path) -> Result<()> {
        let meta = match dest.symlink_metadata() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.entries.push(JournalEntry {
                    original_dest: dest.display().to_string(),
                    backup_path: String::new(),
                    kind: "missing".into(),
                    symlink_target: None,
                });
                self.save()?;
                return Ok(());
            }
            Err(e) => return Err(AnvilError::Backup(format!("{}: {e}", dest.display()))),
        };

        let idx = self.entries.len();
        let backup_name = format!("{:04}", idx);
        let backup_path = self.root.join(&backup_name);

        if meta.file_type().is_symlink() {
            let target = fs::read_link(dest).map_err(|e| AnvilError::Backup(e.to_string()))?;
            fs::write(&backup_path, target.to_string_lossy().as_bytes())
                .map_err(|e| AnvilError::Backup(e.to_string()))?;
            self.entries.push(JournalEntry {
                original_dest: dest.display().to_string(),
                backup_path: backup_path.display().to_string(),
                kind: "symlink".into(),
                symlink_target: Some(target.display().to_string()),
            });
        } else if meta.is_dir() {
            copy_file(dest, &backup_path)?;
            self.entries.push(JournalEntry {
                original_dest: dest.display().to_string(),
                backup_path: backup_path.display().to_string(),
                kind: "dir".into(),
                symlink_target: None,
            });
        } else {
            copy_file(dest, &backup_path)?;
            self.entries.push(JournalEntry {
                original_dest: dest.display().to_string(),
                backup_path: backup_path.display().to_string(),
                kind: "file".into(),
                symlink_target: None,
            });
        }
        self.save()?;
        Ok(())
    }

    /// Record a package this run installed (`original_dest` = name, `backup_path` = manager).
    pub fn record_package(&mut self, name: &str, manager: &str) -> Result<()> {
        self.entries.push(JournalEntry {
            original_dest: name.to_string(),
            backup_path: manager.to_string(),
            kind: "package".into(),
            symlink_target: None,
        });
        self.save()
    }

    /// Record a sysctl drop-in path (backs up existing file or notes missing).
    pub fn backup_sysctl_dropin(&mut self, dest: &Path) -> Result<()> {
        let meta = match dest.symlink_metadata() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.entries.push(JournalEntry {
                    original_dest: dest.display().to_string(),
                    backup_path: String::new(),
                    kind: "sysctl-dropin".into(),
                    symlink_target: Some("missing".into()),
                });
                self.save()?;
                return Ok(());
            }
            Err(e) => return Err(AnvilError::Backup(format!("{}: {e}", dest.display()))),
        };

        let idx = self.entries.len();
        let backup_path = self.root.join(format!("{:04}", idx));
        let _ = meta;
        copy_file(dest, &backup_path)?;
        self.entries.push(JournalEntry {
            original_dest: dest.display().to_string(),
            backup_path: backup_path.display().to_string(),
            kind: "sysctl-dropin".into(),
            symlink_target: None,
        });
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        let path = self.root.join("journal.json");
        let json =
            serde_json::to_string_pretty(self).map_err(|e| AnvilError::Backup(e.to_string()))?;
        fs::write(&path, json).map_err(|e| AnvilError::Backup(e.to_string()))?;
        Ok(())
    }

    /// Remove the journal directory if nothing was recorded (all-skip apply).
    pub fn discard_if_empty(&self) -> Result<()> {
        if !self.entries.is_empty() {
            return Ok(());
        }
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(|e| AnvilError::Backup(e.to_string()))?;
        }
        Ok(())
    }

    pub fn load_latest() -> Result<Option<Self>> {
        let journals = Self::list()?;
        Ok(journals
            .into_iter()
            .rev()
            .find(|j| j.is_active() && !j.is_empty()))
    }

    pub fn load_id(id: &str) -> Result<Option<Self>> {
        let base = backups_dir()?;
        Self::load_from(&base.join(id))
    }

    /// All journals, oldest first. Skips dirs without `journal.json`.
    pub fn list() -> Result<Vec<Self>> {
        let base = backups_dir()?;
        if !base.exists() {
            return Ok(Vec::new());
        }
        let mut dirs: Vec<_> = fs::read_dir(&base)
            .map_err(|e| AnvilError::Backup(e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        dirs.sort_by_key(|e| e.file_name());
        let mut out = Vec::new();
        for d in dirs {
            if let Some(j) = Self::load_from(&d.path())? {
                out.push(j);
            }
        }
        Ok(out)
    }

    pub fn load_from(dir: &Path) -> Result<Option<Self>> {
        let path = dir.join("journal.json");
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&path).map_err(|e| AnvilError::Backup(e.to_string()))?;
        let mut journal: BackupJournal =
            serde_json::from_str(&contents).map_err(|e| AnvilError::Backup(e.to_string()))?;
        journal.root = dir.to_path_buf();
        if journal.status.is_empty() {
            journal.status = STATUS_ACTIVE.into();
        }
        Ok(Some(journal))
    }

    /// Restore all entries from this journal (in reverse order), then mark restored.
    pub fn restore(&mut self) -> Result<usize> {
        let mut count = 0;
        for entry in self.entries.iter().rev() {
            if entry.kind == "package" {
                continue;
            }
            let dest = PathBuf::from(&entry.original_dest);
            match entry.kind.as_str() {
                "missing" => {
                    if dest.symlink_metadata().is_ok() {
                        remove_path(&dest)?;
                    }
                    count += 1;
                }
                "symlink" => {
                    if dest.symlink_metadata().is_ok() {
                        remove_path(&dest)?;
                    }
                    if let Some(target) = &entry.symlink_target {
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent).ok();
                        }
                        std::os::unix::fs::symlink(target, &dest).map_err(|e| {
                            AnvilError::SymlinkFailed {
                                path: dest.clone(),
                                source: e,
                            }
                        })?;
                        count += 1;
                    }
                }
                "file" | "dir" | "sysctl-dropin" => {
                    if entry.symlink_target.as_deref() == Some("missing") {
                        if dest.symlink_metadata().is_ok() {
                            remove_path(&dest)?;
                        }
                        count += 1;
                        continue;
                    }
                    let backup = PathBuf::from(&entry.backup_path);
                    if backup.exists() {
                        if dest.symlink_metadata().is_ok() {
                            remove_path(&dest)?;
                        }
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent).ok();
                        }
                        copy_file(&backup, &dest)?;
                        count += 1;
                    }
                }
                _ => {}
            }
        }
        self.status = STATUS_RESTORED.into();
        self.save()?;
        Ok(count)
    }

    pub fn package_entries(&self) -> impl Iterator<Item = &JournalEntry> {
        self.entries.iter().filter(|e| e.kind == "package")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn backup_and_restore_file() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("important.txt");
        fs::write(&dest, "original").unwrap();

        let backup_root = dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let mut journal = BackupJournal::create_in(&backup_root).unwrap();
        journal.backup_path(&dest).unwrap();
        journal.save().unwrap();

        fs::write(&dest, "changed").unwrap();
        journal.restore().unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original");
    }

    #[test]
    fn backup_symlink() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("t");
        let dest = dir.path().join("link");
        fs::write(&target, "x").unwrap();
        symlink(&target, &dest).unwrap();

        let backup_root = dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let mut journal = BackupJournal::create_in(&backup_root).unwrap();
        journal.backup_path(&dest).unwrap();
        assert_eq!(journal.entries[0].kind, "symlink");
    }

    #[test]
    fn backup_path_persists_journal_before_explicit_save() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("important.txt");
        fs::write(&dest, "original").unwrap();

        let backup_root = dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let mut journal = BackupJournal::create_in(&backup_root).unwrap();
        journal.backup_path(&dest).unwrap();
        let loaded = BackupJournal::load_from(&backup_root.join(&journal.id))
            .unwrap()
            .expect("journal.json must be written after each backup_path");
        assert_eq!(loaded.entries.len(), 1);
    }

    #[test]
    fn back_to_back_journals_do_not_share_a_directory() {
        let dir = tempdir().unwrap();
        let backup_root = dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let a = BackupJournal::create_in(&backup_root).unwrap();
        let b = BackupJournal::create_in(&backup_root).unwrap();
        assert_ne!(
            a.id, b.id,
            "two journals created in the same second must not collide"
        );
        assert_ne!(backup_root.join(&a.id), backup_root.join(&b.id));
    }

    #[test]
    fn restore_marks_journal_so_it_is_not_latest() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("important.txt");
        fs::write(&dest, "original").unwrap();

        let backup_root = dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let mut journal = BackupJournal::create_in(&backup_root).unwrap();
        journal.backup_path(&dest).unwrap();
        journal.save().unwrap();

        fs::write(&dest, "changed").unwrap();
        journal.restore().unwrap();

        let json = fs::read_to_string(backup_root.join(&journal.id).join("journal.json")).unwrap();
        assert!(
            json.contains("restored"),
            "journal.json must record restored status so load_latest skips it; got {json}"
        );
    }

    #[test]
    fn restore_sysctl_dropin_kind() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("99-anvil.conf");
        let backup = dir.path().join("backup.conf");
        fs::write(&backup, "kernel.kptr_restrict = 2\n").unwrap();
        fs::write(&dest, "changed\n").unwrap();

        let mut journal = BackupJournal {
            id: "1".into(),
            created_unix: 1,
            entries: vec![JournalEntry {
                original_dest: dest.display().to_string(),
                backup_path: backup.display().to_string(),
                kind: "sysctl-dropin".into(),
                symlink_target: None,
            }],
            status: STATUS_ACTIVE.into(),
            root: dir.path().to_path_buf(),
        };
        journal.restore().unwrap();
        assert_eq!(
            fs::read_to_string(&dest).unwrap(),
            "kernel.kptr_restrict = 2\n"
        );
    }
}
