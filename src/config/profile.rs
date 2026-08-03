//! Profile resolution and merge.
//!
//! Ordered profile lists are primary. `extends` is syntactic sugar that prepends
//! the parent chain. Later links win on the same `dest`. Hooks concatenate in order.

use std::collections::{HashMap, HashSet};

use crate::config::manifest::{Harden, Hooks, Link, Manifest, Packages, Profile};
use crate::error::{AnvilError, Result};

/// Fully resolved profile content after inheritance merge.
#[derive(Debug, Clone, Default)]
pub struct ResolvedProfile {
    /// Links keyed by dest (last writer wins); order preserved for first-seen dests.
    pub links: Vec<Link>,
    pub hooks: Hooks,
    pub packages: Packages,
    pub harden: Option<Harden>,
    /// Profile names that contributed, in merge order.
    pub chain: Vec<String>,
}

/// Resolve and merge an ordered list of profile names.
pub fn resolve_profiles(manifest: &Manifest, names: &[String]) -> Result<ResolvedProfile> {
    if names.is_empty() {
        return Err(AnvilError::Other(
            "no profiles specified; set default_profile, pass -p, or configure [machines]".into(),
        ));
    }

    let mut resolved = ResolvedProfile::default();
    let mut link_index: HashMap<String, usize> = HashMap::new();

    for name in names {
        let chain = expand_extends(manifest, name)?;
        for profile_name in chain {
            if resolved.chain.last() == Some(&profile_name) {
                // avoid duplicate consecutive from multi-select + extends
            }
            if !resolved.chain.contains(&profile_name) {
                resolved.chain.push(profile_name.clone());
            }
            let profile = manifest.get_profile(&profile_name)?;
            merge_profile_into(&mut resolved, &mut link_index, profile);
        }
    }

    Ok(resolved)
}

/// Expand `extends` chain for a single profile (parents first, then self).
/// Detects cycles.
fn expand_extends(manifest: &Manifest, name: &str) -> Result<Vec<String>> {
    let mut chain = Vec::new();
    let mut visited = HashSet::new();
    walk_extends(manifest, name, &mut visited, &mut chain)?;
    Ok(chain)
}

fn walk_extends(
    manifest: &Manifest,
    name: &str,
    visited: &mut HashSet<String>,
    chain: &mut Vec<String>,
) -> Result<()> {
    if !visited.insert(name.to_string()) {
        return Err(AnvilError::ProfileCycle(name.to_string()));
    }
    let profile = manifest.get_profile(name)?;
    if let Some(parent) = &profile.extends {
        walk_extends(manifest, parent, visited, chain)?;
    }
    chain.push(name.to_string());
    Ok(())
}

fn merge_profile_into(
    resolved: &mut ResolvedProfile,
    link_index: &mut HashMap<String, usize>,
    profile: &Profile,
) {
    for link in &profile.links {
        if let Some(&idx) = link_index.get(&link.dest) {
            resolved.links[idx] = link.clone();
        } else {
            link_index.insert(link.dest.clone(), resolved.links.len());
            resolved.links.push(link.clone());
        }
    }

    if let Some(hooks) = &profile.hooks {
        if let Some(before) = &hooks.before_apply {
            resolved
                .hooks
                .before_apply
                .get_or_insert_with(Vec::new)
                .extend(before.iter().cloned());
        }
        if let Some(after) = &hooks.after_apply {
            resolved
                .hooks
                .after_apply
                .get_or_insert_with(Vec::new)
                .extend(after.iter().cloned());
        }
    }

    if let Some(packages) = &profile.packages {
        merge_packages(&mut resolved.packages, packages);
    }

    if profile.harden.is_some() {
        // later harden config wins wholesale (simple model)
        resolved.harden = profile.harden.clone();
    }
}

fn merge_packages(into: &mut Packages, from: &Packages) {
    if let Some(pacman) = &from.pacman {
        into.pacman
            .get_or_insert_with(Vec::new)
            .extend(pacman.iter().cloned());
        if let Some(list) = &mut into.pacman {
            list.sort();
            list.dedup();
        }
    }
    if let Some(aur) = &from.aur {
        into.aur
            .get_or_insert_with(Vec::new)
            .extend(aur.iter().cloned());
        if let Some(list) = &mut into.aur {
            list.sort();
            list.dedup();
        }
    }
}

/// Pick profile names for this machine from CLI, local config, machines table, or default.
pub fn select_profile_names(
    manifest: &Manifest,
    cli_profiles: &[String],
    local_profiles: &[String],
    hostname: &str,
) -> Result<Vec<String>> {
    if !cli_profiles.is_empty() {
        return Ok(cli_profiles.to_vec());
    }
    if !local_profiles.is_empty() {
        return Ok(local_profiles.to_vec());
    }
    if let Some(machines) = &manifest.machines
        && let Some(profiles) = machines.get(hostname)
        && !profiles.is_empty()
    {
        return Ok(profiles.clone());
    }
    if let Some(default) = manifest.default_profile_name() {
        return Ok(vec![default.to_string()]);
    }
    // single profile named base, or first key
    if manifest.profiles.contains_key("base") {
        return Ok(vec!["base".into()]);
    }
    if let Some(name) = manifest.profiles.keys().next() {
        return Ok(vec![name.clone()]);
    }
    Err(AnvilError::Other(
        "no profiles defined in anvil.toml".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml: &str) -> Manifest {
        Manifest::parse_toml(toml).unwrap()
    }

    #[test]
    fn extends_merges_parent_then_child() {
        let m = parse(
            r#"
[anvil]
version = "1"

[profiles.base]
links = [
  { src = ".zshrc", dest = "~/.zshrc" },
  { src = ".gitconfig", dest = "~/.gitconfig" },
]

[profiles.work]
extends = "base"
links = [
  { src = "work/.gitconfig", dest = "~/.gitconfig" },
]
"#,
        );
        let r = resolve_profiles(&m, &["work".into()]).unwrap();
        assert_eq!(r.links.len(), 2);
        let git = r.links.iter().find(|l| l.dest == "~/.gitconfig").unwrap();
        assert_eq!(git.src, "work/.gitconfig");
        assert!(r.chain.contains(&"base".to_string()));
        assert!(r.chain.contains(&"work".to_string()));
    }

    #[test]
    fn cycle_detected() {
        let m = parse(
            r#"
[anvil]
version = "1"

[profiles.a]
extends = "b"
links = []

[profiles.b]
extends = "a"
links = []
"#,
        );
        let err = resolve_profiles(&m, &["a".into()]).unwrap_err();
        assert!(matches!(err, AnvilError::ProfileCycle(_)));
    }

    #[test]
    fn multi_profile_later_wins() {
        let m = parse(
            r#"
[anvil]
version = "1"

[profiles.base]
links = [{ src = "a", dest = "~/.x" }]

[profiles.extra]
links = [{ src = "b", dest = "~/.x" }]
"#,
        );
        let r = resolve_profiles(&m, &["base".into(), "extra".into()]).unwrap();
        assert_eq!(r.links.len(), 1);
        assert_eq!(r.links[0].src, "b");
    }

    #[test]
    fn hooks_concatenate() {
        let m = parse(
            r#"
[anvil]
version = "1"

[profiles.base]
hooks.before_apply = ["a.sh"]
hooks.after_apply = ["b.sh"]

[profiles.extra]
extends = "base"
hooks.after_apply = ["c.sh"]
"#,
        );
        let r = resolve_profiles(&m, &["extra".into()]).unwrap();
        assert_eq!(
            r.hooks.before_apply.as_ref().unwrap(),
            &["a.sh".to_string()]
        );
        assert_eq!(
            r.hooks.after_apply.as_ref().unwrap(),
            &["b.sh".to_string(), "c.sh".to_string()]
        );
    }

    #[test]
    fn packages_merge_dedup() {
        let m = parse(
            r#"
[anvil]
version = "1"

[profiles.base]
packages.pacman = ["git", "ufw"]

[profiles.extra]
extends = "base"
packages.pacman = ["ufw", "age"]
"#,
        );
        let r = resolve_profiles(&m, &["extra".into()]).unwrap();
        let pkgs = r.packages.pacman.unwrap();
        assert_eq!(pkgs, vec!["age", "git", "ufw"]);
    }

    #[test]
    fn select_cli_wins() {
        let m = parse(
            r#"
[anvil]
version = "1"
default_profile = "base"
[profiles.base]
links = []
[profiles.work]
links = []
"#,
        );
        let names = select_profile_names(&m, &["work".into()], &["base".into()], "host").unwrap();
        assert_eq!(names, vec!["work"]);
    }

    #[test]
    fn select_machines() {
        let m = parse(
            r#"
[anvil]
version = "1"
[profiles.base]
links = []
[profiles.hyprland]
links = []
[machines]
"framework-arch" = ["base", "hyprland"]
"#,
        );
        let names = select_profile_names(&m, &[], &[], "framework-arch").unwrap();
        assert_eq!(names, vec!["base", "hyprland"]);
    }
}
