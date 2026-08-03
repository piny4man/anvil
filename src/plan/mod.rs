//! Build a mutation plan before touching the filesystem.

use std::path::Path;

use crate::config::ResolvedProfile;
use crate::config::manifest::Link;
use crate::error::Result;
use crate::linker::{LinkStatus, ResolvedLink, default_mode_for, inspect, parse_mode};
use crate::paths::{expand_path, expand_path_relative};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    Create,
    SkipCorrect,
    Conflict,
    Broken,
    Decrypt,
}

#[derive(Debug, Clone)]
pub struct FileOp {
    pub link: ResolvedLink,
    pub action: FileAction,
    pub display_src: String,
    pub display_dest: String,
}

#[derive(Debug, Clone)]
pub struct PackageOp {
    pub name: String,
    pub manager: String, // pacman | aur
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct HardenOp {
    pub id: String,
    pub description: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub file_ops: Vec<FileOp>,
    pub package_ops: Vec<PackageOp>,
    pub harden_ops: Vec<HardenOp>,
    pub before_hooks: Vec<String>,
    pub after_hooks: Vec<String>,
    pub profiles: Vec<String>,
}

impl Plan {
    pub fn files_to_apply(&self) -> impl Iterator<Item = &FileOp> {
        self.file_ops.iter().filter(|op| {
            matches!(
                op.action,
                FileAction::Create
                    | FileAction::Conflict
                    | FileAction::Broken
                    | FileAction::Decrypt
            )
        })
    }

    pub fn conflicts(&self) -> impl Iterator<Item = &FileOp> {
        self.file_ops
            .iter()
            .filter(|op| op.action == FileAction::Conflict)
    }

    pub fn packages_missing(&self) -> impl Iterator<Item = &PackageOp> {
        self.package_ops.iter().filter(|p| !p.installed)
    }

    pub fn harden_failing(&self) -> impl Iterator<Item = &HardenOp> {
        self.harden_ops.iter().filter(|h| !h.ok)
    }
}

/// Build a full plan from a resolved profile and repo root.
pub fn build_plan(
    repo: &Path,
    resolved: &ResolvedProfile,
    check_packages: bool,
    check_harden: bool,
) -> Result<Plan> {
    let mut plan = Plan {
        profiles: resolved.chain.clone(),
        before_hooks: resolved.hooks.before_apply.clone().unwrap_or_default(),
        after_hooks: resolved.hooks.after_apply.clone().unwrap_or_default(),
        ..Plan::default()
    };

    for link in &resolved.links {
        plan.file_ops.push(resolve_file_op(repo, link)?);
    }

    if check_packages {
        plan.package_ops = crate::packages::plan_packages(&resolved.packages)?;
    }

    if check_harden && let Some(harden) = &resolved.harden {
        plan.harden_ops = crate::harden::check_all(harden);
    }

    Ok(plan)
}

fn resolve_file_op(repo: &Path, link: &Link) -> Result<FileOp> {
    let src = expand_path_relative(&link.src, repo)?;
    let dest = expand_path(&link.dest)?;
    let mode = match &link.mode {
        Some(m) => Some(parse_mode(m)?),
        None => default_mode_for(&dest),
    };
    let copy = link.copy.unwrap_or(false) || link.decrypt.is_some();
    let resolved = ResolvedLink {
        src: src.clone(),
        dest: dest.clone(),
        copy,
        mode,
        decrypt: link.decrypt.clone(),
    };

    if !src.exists() && link.decrypt.is_none() {
        // still plan it; apply will fail — or mark broken source
        return Ok(FileOp {
            action: FileAction::Broken,
            display_src: link.src.clone(),
            display_dest: link.dest.clone(),
            link: resolved,
        });
    }

    // for age files, src should exist as .age
    if link.decrypt.is_some() && !src.exists() {
        return Ok(FileOp {
            action: FileAction::Broken,
            display_src: link.src.clone(),
            display_dest: link.dest.clone(),
            link: resolved,
        });
    }

    let status = inspect(&resolved)?;
    let action = if link.decrypt.is_some() {
        match status {
            LinkStatus::Correct => FileAction::SkipCorrect, // content match hard for decrypt; treat conflict otherwise
            LinkStatus::Missing => FileAction::Decrypt,
            LinkStatus::Broken => FileAction::Decrypt,
            LinkStatus::Conflict => FileAction::Conflict,
        }
    } else {
        match status {
            LinkStatus::Missing => FileAction::Create,
            LinkStatus::Correct => FileAction::SkipCorrect,
            LinkStatus::Conflict => FileAction::Conflict,
            LinkStatus::Broken => FileAction::Broken,
        }
    };

    Ok(FileOp {
        link: resolved,
        action,
        display_src: link.src.clone(),
        display_dest: link.dest.clone(),
    })
}

/// Format plan for terminal display (no colors — ui layer may restyle).
pub fn format_plan_summary(plan: &Plan) -> String {
    let mut lines = Vec::new();
    let create = plan
        .file_ops
        .iter()
        .filter(|o| matches!(o.action, FileAction::Create | FileAction::Decrypt))
        .count();
    let skip = plan
        .file_ops
        .iter()
        .filter(|o| o.action == FileAction::SkipCorrect)
        .count();
    let conflicts = plan
        .file_ops
        .iter()
        .filter(|o| o.action == FileAction::Conflict)
        .count();
    let broken = plan
        .file_ops
        .iter()
        .filter(|o| o.action == FileAction::Broken)
        .count();
    lines.push(format!(
        "FILES     {create} to link, {skip} ok, {conflicts} conflict, {broken} broken"
    ));

    let missing: Vec<_> = plan.packages_missing().map(|p| p.name.as_str()).collect();
    if !plan.package_ops.is_empty() {
        lines.push(format!(
            "PACKAGES  {} missing{}",
            missing.len(),
            if missing.is_empty() {
                String::new()
            } else {
                format!(" ({})", missing.join(", "))
            }
        ));
    }

    let fail: Vec<_> = plan.harden_failing().collect();
    if !plan.harden_ops.is_empty() {
        lines.push(format!(
            "HARDEN    {} check(s) failing of {}",
            fail.len(),
            plan.harden_ops.len()
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::manifest::Manifest;
    use crate::config::resolve_profiles;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn plan_create_and_skip() {
        let dir = tempdir().unwrap();
        let repo = dir.path();
        fs::write(repo.join(".zshrc"), "rc").unwrap();
        let home_link = dir.path().join("home");
        fs::create_dir_all(&home_link).unwrap();

        // Use absolute dest under temp
        let dest = home_link.join("zshrc");
        let toml = format!(
            r#"
[anvil]
version = "1"
[profiles.base]
links = [
  {{ src = ".zshrc", dest = "{}" }},
]
"#,
            dest.display()
        );
        let m = Manifest::parse_toml(&toml).unwrap();
        let resolved = resolve_profiles(&m, &["base".into()]).unwrap();
        let plan = build_plan(repo, &resolved, false, false).unwrap();
        assert_eq!(plan.file_ops.len(), 1);
        assert_eq!(plan.file_ops[0].action, FileAction::Create);

        // apply symlink manually
        std::os::unix::fs::symlink(repo.join(".zshrc"), &dest).unwrap();
        let plan2 = build_plan(repo, &resolved, false, false).unwrap();
        assert_eq!(plan2.file_ops[0].action, FileAction::SkipCorrect);
    }
}
