use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{LocalConfig, Manifest, discover_repo};
use crate::error::{AnvilError, Result};
use crate::linker::{BackupJournal, ResolvedLink, apply_link};
use crate::ui::UiContext;

pub fn run(file: PathBuf, profile: Option<String>, ctx: &UiContext) -> Result<()> {
    let local = LocalConfig::load()?;
    let repo = discover_repo(&local)?;
    let repo = fs::canonicalize(&repo).unwrap_or(repo);
    let manifest_path = repo.join("anvil.toml");
    let manifest = Manifest::from_path(&manifest_path)?;

    let abs = if file.is_absolute() {
        file.clone()
    } else {
        std::env::current_dir()
            .map_err(|e| AnvilError::Io {
                path: file.clone(),
                source: e,
            })?
            .join(&file)
    };

    let meta = abs.symlink_metadata().map_err(|e| AnvilError::Io {
        path: abs.clone(),
        source: e,
    })?;

    if meta.file_type().is_symlink() && symlink_points_into_repo(&abs, &repo)? {
        return Err(AnvilError::Other(format!(
            "already managed by this repo: {}",
            abs.display()
        )));
    }

    // Follow only real files/dirs so dest stays the user-facing path for symlinks we refuse above.
    let abs = if meta.file_type().is_symlink() {
        abs
    } else {
        fs::canonicalize(&abs).unwrap_or(abs)
    };

    if abs.starts_with(&repo) {
        return Err(AnvilError::Other(format!(
            "path is already inside the dotfiles repo: {}",
            abs.display()
        )));
    }

    let profile_name = match profile {
        Some(p) => p,
        None => {
            let mut names: Vec<String> = manifest.profiles.keys().cloned().collect();
            names.sort();
            if names.is_empty() {
                return Err(AnvilError::Other("no profiles in anvil.toml".into()));
            }
            if ctx.yes {
                names
                    .first()
                    .cloned()
                    .or_else(|| manifest.default_profile_name().map(|s| s.to_string()))
                    .ok_or_else(|| AnvilError::Other("no profile".into()))?
            } else {
                let idx = ctx.select("Add to profile", names.clone(), 0)?;
                names[idx].clone()
            }
        }
    };

    let copy = if ctx.yes {
        false
    } else {
        let idx = ctx.select("Link mode", vec!["symlink", "copy"], 0)?;
        idx == 1
    };

    let home = dirs::home_dir().ok_or(AnvilError::HomeDirNotFound)?;
    let rel = abs
        .strip_prefix(&home)
        .map(|p| PathBuf::from(".").join(p))
        .unwrap_or_else(|_| PathBuf::from("adopted").join(abs.file_name().unwrap_or_default()));
    let rel_str = rel.to_string_lossy().trim_start_matches("./").to_string();
    let dest_in_repo = repo.join(&rel_str);

    let dest_display = if abs.starts_with(&home) {
        format!("~/{}", abs.strip_prefix(&home).unwrap().to_string_lossy())
    } else {
        abs.display().to_string()
    };

    if dest_in_repo.exists() {
        return Err(AnvilError::Other(format!(
            "already exists in repo: {}",
            dest_in_repo.display()
        )));
    }

    if ctx.dry_run {
        ctx.success(&format!(
            "would move {} → {}",
            abs.display(),
            dest_in_repo.display()
        ));
        ctx.success(&format!(
            "would add link to profile `{profile_name}` (copy={copy})"
        ));
        return Ok(());
    }

    let mut journal = BackupJournal::create()?;
    journal.backup_path(&abs)?;

    if let Some(parent) = dest_in_repo.parent() {
        fs::create_dir_all(parent).map_err(|e| AnvilError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    if let Err(e) = adopt_into_repo(&abs, &dest_in_repo) {
        let _ = journal.restore();
        return Err(e);
    }

    let link = ResolvedLink {
        src: dest_in_repo.clone(),
        dest: abs.clone(),
        copy,
        mode: None,
        decrypt: None,
    };
    if let Err(e) = apply_link(&link, false) {
        let _ = journal.restore();
        let _ = crate::linker::remove_path(&dest_in_repo);
        return Err(e);
    }

    ctx.success(&format!(
        "Moved   {} → {}",
        abs.display(),
        dest_in_repo.display()
    ));
    ctx.success(&format!(
        "Linked  {} → {}",
        abs.display(),
        dest_in_repo.display()
    ));

    if let Err(e) =
        append_link_to_manifest(&manifest_path, &profile_name, &rel_str, &dest_display, copy)
    {
        let _ = journal.restore();
        return Err(e);
    }
    ctx.success("Updated anvil.toml");
    ctx.warn(&format!(
        "Don't forget to commit: cd {} && git add . && git commit",
        repo.display()
    ));
    Ok(())
}

/// Copy into the repo and verify. Does not delete `abs` — `apply_link` replaces dest after backup.
fn adopt_into_repo(abs: &Path, dest_in_repo: &Path) -> Result<()> {
    if abs.is_dir() {
        crate::linker::copy_file(abs, dest_in_repo)?;
    } else {
        fs::copy(abs, dest_in_repo).map_err(|e| AnvilError::CopyFailed {
            path: dest_in_repo.to_path_buf(),
            source: e,
        })?;
        let orig = fs::read(abs).map_err(|e| AnvilError::Io {
            path: abs.to_path_buf(),
            source: e,
        })?;
        let copied = fs::read(dest_in_repo).map_err(|e| AnvilError::Io {
            path: dest_in_repo.to_path_buf(),
            source: e,
        })?;
        if orig != copied {
            let _ = crate::linker::remove_path(dest_in_repo);
            return Err(AnvilError::CopyFailed {
                path: dest_in_repo.to_path_buf(),
                source: std::io::Error::other("copy verification failed"),
            });
        }
    }
    Ok(())
}

fn symlink_points_into_repo(path: &Path, repo: &Path) -> Result<bool> {
    let target = fs::read_link(path).map_err(|e| AnvilError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let resolved = if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or(Path::new("/")).join(target)
    };
    let canonical = fs::canonicalize(&resolved).unwrap_or(resolved);
    Ok(canonical.starts_with(repo))
}

fn append_link_to_manifest(
    path: &Path,
    profile: &str,
    src: &str,
    dest: &str,
    copy: bool,
) -> Result<()> {
    let contents = fs::read_to_string(path).map_err(|e| AnvilError::ConfigRead {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut doc: toml_edit::DocumentMut = contents
        .parse()
        .map_err(|e: toml_edit::TomlError| AnvilError::ConfigParse(e.to_string()))?;

    let profiles = doc
        .get_mut("profiles")
        .and_then(|i| i.as_table_like_mut())
        .ok_or_else(|| AnvilError::ConfigParse("missing [profiles]".into()))?;

    let profile_item = profiles
        .get_mut(profile)
        .ok_or_else(|| AnvilError::ProfileNotFound(profile.to_string()))?;

    let table = profile_item
        .as_table_like_mut()
        .ok_or_else(|| AnvilError::ConfigParse(format!("profile `{profile}` is not a table")))?;

    let links = table
        .entry("links")
        .or_insert(toml_edit::Item::Value(toml_edit::Value::Array(
            toml_edit::Array::new(),
        )));

    let arr = links
        .as_array_mut()
        .ok_or_else(|| AnvilError::ConfigParse("links is not an array".into()))?;

    let mut inline = toml_edit::InlineTable::new();
    inline.insert("src", src.into());
    inline.insert("dest", dest.into());
    if copy {
        inline.insert("copy", true.into());
    }
    arr.push(inline);

    fs::write(path, doc.to_string()).map_err(|e| AnvilError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}
