//! Symlink / copy operations and backup journal.

mod backup;

pub use backup::{BackupJournal, JournalEntry};

use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::error::{AnvilError, Result};

/// Desired file operation after path resolution.
#[derive(Debug, Clone)]
pub struct ResolvedLink {
    pub src: PathBuf,
    pub dest: PathBuf,
    pub copy: bool,
    pub mode: Option<u32>,
    /// When set, `src` is encrypted and must be decrypted to dest.
    pub decrypt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// Dest missing — needs create.
    Missing,
    /// Already correct symlink or identical copy.
    Correct,
    /// Symlink exists but points elsewhere / wrong type.
    Conflict,
    /// Broken symlink.
    Broken,
}

/// Inspect current state of dest relative to desired src.
pub fn inspect(link: &ResolvedLink) -> Result<LinkStatus> {
    let dest = &link.dest;
    if !dest.exists() && !dest.symlink_metadata().is_ok() {
        // truly missing
        if dest.symlink_metadata().is_err() {
            return Ok(LinkStatus::Missing);
        }
    }

    let meta = match dest.symlink_metadata() {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(LinkStatus::Missing),
        Err(e) => {
            return Err(AnvilError::Io {
                path: dest.clone(),
                source: e,
            });
        }
    };

    if meta.file_type().is_symlink() {
        match fs::read_link(dest) {
            Ok(target) => {
                let canonical_target = if target.is_absolute() {
                    target
                } else {
                    dest.parent().unwrap_or(Path::new("/")).join(target)
                };
                // Compare as paths (may not exist if broken)
                if !dest.exists() {
                    return Ok(LinkStatus::Broken);
                }
                if paths_equal(&canonical_target, &link.src)
                    || paths_equal(
                        &fs::canonicalize(dest).unwrap_or(canonical_target.clone()),
                        &link.src,
                    )
                {
                    // also accept if canonicalize matches
                    if let (Ok(a), Ok(b)) = (fs::canonicalize(dest), fs::canonicalize(&link.src))
                        && a == b
                    {
                        return Ok(LinkStatus::Correct);
                    }
                    if canonical_target == link.src {
                        return Ok(LinkStatus::Correct);
                    }
                    // read_link exact match
                    if fs::read_link(dest).ok().as_ref() == Some(&link.src) {
                        return Ok(LinkStatus::Correct);
                    }
                    return Ok(LinkStatus::Conflict);
                }
                if fs::read_link(dest).ok().as_ref() == Some(&link.src) {
                    return Ok(LinkStatus::Correct);
                }
                // broken if target missing
                if !canonical_target.exists() && !link.src.exists() {
                    return Ok(LinkStatus::Broken);
                }
                Ok(LinkStatus::Conflict)
            }
            Err(_) => Ok(LinkStatus::Broken),
        }
    } else if link.copy {
        // compare contents
        if files_identical(&link.src, dest)? {
            Ok(LinkStatus::Correct)
        } else {
            Ok(LinkStatus::Conflict)
        }
    } else {
        // regular file/dir where we want symlink
        Ok(LinkStatus::Conflict)
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a == b
}

fn files_identical(a: &Path, b: &Path) -> Result<bool> {
    if !a.is_file() || !b.is_file() {
        return Ok(false);
    }
    let ca = fs::read(a).map_err(|e| AnvilError::Io {
        path: a.to_path_buf(),
        source: e,
    })?;
    let cb = fs::read(b).map_err(|e| AnvilError::Io {
        path: b.to_path_buf(),
        source: e,
    })?;
    Ok(ca == cb)
}

/// Apply a resolved link (create symlink or copy). Caller handles conflicts/backups.
pub fn apply_link(link: &ResolvedLink, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }

    if let Some(parent) = link.dest.parent() {
        fs::create_dir_all(parent).map_err(|e| AnvilError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    if link.copy || link.decrypt.is_some() {
        // content already written by secrets layer for decrypt; else copy
        if link.decrypt.is_none() {
            copy_file(&link.src, &link.dest)?;
        }
        if let Some(mode) = link.mode {
            set_mode(&link.dest, mode)?;
        }
    } else {
        // remove dest if exists (caller should have backed up)
        if link.dest.symlink_metadata().is_ok() {
            remove_path(&link.dest)?;
        }
        symlink(&link.src, &link.dest).map_err(|e| AnvilError::SymlinkFailed {
            path: link.dest.clone(),
            source: e,
        })?;
        // mode on symlink is less meaningful; apply to target if file and mode set
        if let Some(mode) = link.mode
            && link.src.is_file()
        {
            set_mode(&link.src, mode)?;
        }
    }
    Ok(())
}

/// Write bytes to dest with optional mode (used after age decrypt).
pub fn write_file(dest: &Path, data: &[u8], mode: Option<u32>, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| AnvilError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    if dest.symlink_metadata().is_ok() {
        remove_path(dest)?;
    }
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    if let Some(mode) = mode {
        opts.mode(mode);
    }
    use std::io::Write;
    let mut f = opts.open(dest).map_err(|e| AnvilError::Io {
        path: dest.to_path_buf(),
        source: e,
    })?;
    f.write_all(data).map_err(|e| AnvilError::Io {
        path: dest.to_path_buf(),
        source: e,
    })?;
    if let Some(mode) = mode {
        set_mode(dest, mode)?;
    }
    Ok(())
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    if dest.symlink_metadata().is_ok() {
        remove_path(dest)?;
    }
    if src.is_dir() {
        copy_dir_all(src, dest)?;
    } else {
        fs::copy(src, dest).map_err(|e| AnvilError::CopyFailed {
            path: dest.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|e| AnvilError::Io {
        path: dest.to_path_buf(),
        source: e,
    })?;
    for entry in fs::read_dir(src).map_err(|e| AnvilError::Io {
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| AnvilError::Io {
            path: src.to_path_buf(),
            source: e,
        })?;
        let ty = entry.file_type().map_err(|e| AnvilError::Io {
            path: entry.path(),
            source: e,
        })?;
        let to = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to).map_err(|e| AnvilError::CopyFailed {
                path: to,
                source: e,
            })?;
        }
    }
    Ok(())
}

pub fn remove_path(path: &Path) -> Result<()> {
    let meta = path.symlink_metadata().map_err(|e| AnvilError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(path).map_err(|e| AnvilError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
    } else {
        fs::remove_file(path).map_err(|e| AnvilError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms).map_err(|e| AnvilError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Parse octal mode string like "600" or "0600".
pub fn parse_mode(s: &str) -> Result<u32> {
    u32::from_str_radix(s.trim(), 8).map_err(|_| {
        AnvilError::InvalidPath(format!("invalid mode `{s}` (expected octal like 600)"))
    })
}

/// Infer restrictive mode for sensitive paths when not specified.
pub fn default_mode_for(dest: &Path) -> Option<u32> {
    let s = dest.to_string_lossy();
    if s.contains("/.ssh/") || s.ends_with("/.ssh") {
        return Some(0o600);
    }
    if s.ends_with(".age") || s.contains("/secrets/") {
        return Some(0o600);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn symlink_create_and_inspect_correct() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        fs::write(&src, "hello").unwrap();

        let link = ResolvedLink {
            src: src.clone(),
            dest: dest.clone(),
            copy: false,
            mode: None,
            decrypt: None,
        };
        assert_eq!(inspect(&link).unwrap(), LinkStatus::Missing);
        apply_link(&link, false).unwrap();
        assert_eq!(inspect(&link).unwrap(), LinkStatus::Correct);
        assert_eq!(fs::read_link(&dest).unwrap(), src);
    }

    #[test]
    fn conflict_when_regular_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        fs::write(&src, "a").unwrap();
        fs::write(&dest, "b").unwrap();
        let link = ResolvedLink {
            src,
            dest,
            copy: false,
            mode: None,
            decrypt: None,
        };
        assert_eq!(inspect(&link).unwrap(), LinkStatus::Conflict);
    }

    #[test]
    fn copy_mode_identical() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        fs::write(&src, "same").unwrap();
        fs::write(&dest, "same").unwrap();
        let link = ResolvedLink {
            src,
            dest,
            copy: true,
            mode: None,
            decrypt: None,
        };
        assert_eq!(inspect(&link).unwrap(), LinkStatus::Correct);
    }

    #[test]
    fn parse_mode_octal() {
        assert_eq!(parse_mode("600").unwrap(), 0o600);
        assert_eq!(parse_mode("0644").unwrap(), 0o644);
    }

    #[test]
    fn default_mode_ssh() {
        assert_eq!(
            default_mode_for(Path::new("/home/u/.ssh/config")),
            Some(0o600)
        );
    }
}
