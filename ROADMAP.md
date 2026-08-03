# Roadmap

This document tracks the implementation plan for anvil.

---

## Phase 1′ — Dogfoodable file manager

- [x] **Foundation** — Error types, manifest structs, `deny_unknown_fields`
- [x] **CLI Skeleton** — Clap derive + global flags (`--yes`, `--quiet`, `--dry-run`, `--force`)
- [x] **UI Module** — `UiContext`, theme, spinner, prompts, summary
- [x] **Safe `--yes` defaults** — multi_select empty; conflict Skip unless `--force`
- [x] **Local state + path expand** — XDG config/state, `expand_path`, discover repo
- [x] **Profile merge** — `extends` + ordered lists, cycle detection, later wins on dest
- [x] **Linker + backup journal** — symlink/copy, inspect, backups, `anvil undo`
- [x] **Plan-based apply** — dry-run first-class, conflict prompt, summary
- [x] **Git backend** — `GitBackend` + `ShellGit`
- [x] **init / sync / status / doctor / add / undo**
- [x] **Hooks** — streamed output, repo-relative only

## Phase 2′ — Secrets & packages

- [x] **age secrets** — `decrypt = "age"` on links; identity in local config
- [x] **packages plane** — pacman + AUR (paru/yay); `anvil apply --packages`
- [ ] Machine table auto-write on init (optional commit of hostname → profiles)
- [ ] Deeper `add` edge cases (already-linked, directory adopt polish)

## Phase 3′ — Hardening plane

- [x] **harden checks** — sysctl, sshd, ufw, home perms; `anvil apply --harden` / doctor
- [x] **enforce path** — sysctl.d drop-in when `mode = "enforce"` (sudo)
- [ ] Curated Arch recipe profiles (docs/examples)
- [ ] Shell completions via clap
- [ ] Cross-platform release binaries

## Phase 4′ — Multi-distro & polish

- [ ] apt/dnf package backends
- [ ] Optional light templates (`hostname`, `home`, `os`)
- [ ] libgit2 backend
- [ ] Publish to crates.io

---

## Design Decisions

- **`UiContext` by `&` reference** — no globals; `--yes` ≠ `--force`
- **`deny_unknown_fields`** on all config structs
- **`GitBackend` trait** — ShellGit default
- **Plan → confirm → execute** — one path for dry-run and apply
- **XDG local config** — `~/.config/anvil/config.toml` + state under `~/.local/state/anvil/`
- **Tilde expansion** via `dirs::home_dir()` only (`~` and `~/...`)
- **Hooks** stream with `│ ` prefix; absolute paths and `..` rejected
- **Integration tests** use tempfile + XDG env overrides — never the real home tree
