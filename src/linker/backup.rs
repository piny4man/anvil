//! Backup journal for undo support.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{copy_file, remove_path};
use crate::error::{AnvilError, Result};
use crate::paths::backups_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub original_dest: String,
    pub backup_path: String,
    pub kind: String, // "file" | "symlink" | "missing"
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJournal {
    pub id: String,
    pub created_unix: u64,
    pub entries: Vec<JournalEntry>,
    #[serde(skip)]
    root: PathBuf,
}

impl BackupJournal {
    pub fn create() -> Result<Self> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let id = format!("{ts}");
        let root = backups_dir()?.join(&id);
        fs::create_dir_all(&root).map_err(|e| AnvilError::Backup(e.to_string()))?;
        Ok(Self {
            id,
            created_unix: ts,
            entries: Vec::new(),
            root,
        })
    }

    /// Create journal under a custom root (tests).
    pub fn create_in(base: &Path) -> Result<Self> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let id = format!("{ts}");
        let root = base.join(&id);
        fs::create_dir_all(&root).map_err(|e| AnvilError::Backup(e.to_string()))?;
        Ok(Self {
            id,
            created_unix: ts,
            entries: Vec::new(),
            root,
        })
    }

    /// Backup existing path before overwrite. No-op if path does not exist.
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
                return Ok(());
            }
            Err(e) => return Err(AnvilError::Backup(format!("{}: {e}", dest.display()))),
        };

        let idx = self.entries.len();
        let backup_name = format!("{:04}", idx);
        let backup_path = self.root.join(&backup_name);

        if meta.file_type().is_symlink() {
            let target = fs::read_link(dest).map_err(|e| AnvilError::Backup(e.to_string()))?;
            // store target in a sidecar text file
            fs::write(&backup_path, target.to_string_lossy().as_bytes())
                .map_err(|e| AnvilError::Backup(e.to_string()))?;
            self.entries.push(JournalEntry {
                original_dest: dest.display().to_string(),
                backup_path: backup_path.display().to_string(),
                kind: "symlink".into(),
                symlink_target: Some(target.display().to_string()),
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
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let path = self.root.join("journal.json");
        let json =
            serde_json::to_string_pretty(self).map_err(|e| AnvilError::Backup(e.to_string()))?;
        fs::write(&path, json).map_err(|e| AnvilError::Backup(e.to_string()))?;
        Ok(())
    }

    pub fn load_latest() -> Result<Option<Self>> {
        let base = backups_dir()?;
        if !base.exists() {
            return Ok(None);
        }
        let mut dirs: Vec<_> = fs::read_dir(&base)
            .map_err(|e| AnvilError::Backup(e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        dirs.sort_by_key(|e| e.file_name());
        let Some(last) = dirs.last() else {
            return Ok(None);
        };
        Self::load_from(&last.path())
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
        Ok(Some(journal))
    }

    /// Restore all entries from this journal (in reverse order).
    pub fn restore(&self) -> Result<usize> {
        let mut count = 0;
        for entry in self.entries.iter().rev() {
            let dest = PathBuf::from(&entry.original_dest);
            if dest.symlink_metadata().is_ok() {
                remove_path(&dest)?;
            }
            match entry.kind.as_str() {
                "missing" => {
                    // leave removed
                }
                "symlink" => {
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
                "file" => {
                    let backup = PathBuf::from(&entry.backup_path);
                    if backup.exists() {
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
        Ok(count)
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
}
