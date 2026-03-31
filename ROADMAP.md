# Roadmap

This document tracks the implementation plan for anvil. The project follows a phased approach, building from core infrastructure toward a complete interactive dotfiles manager.

---

## Phase 1 — Interactive MVP

The MVP delivers `init`, `sync`, and `apply` with interactive prompts, symlink management, and basic profile support. Broken into 9 independently compilable steps.

### Dependency graph

```
Step 1  Foundation
  │
  ├──► Step 2  CLI Skeleton ──────────┐
  ├──► Step 3  UI Module ─────────────┤
  ├──► Step 4  Git Backend ───────────┤
  └──► Step 5  Linker ────────────────┤
                                      │
               ┌──────────────────────┘
               ▼
         Step 6  apply command
               │
               ├──► Step 7  init command
               ├──► Step 8  sync command
               │
               └──► Step 9  Polish & edge cases
```

Steps 2–5 can proceed in parallel after Step 1. Step 6 unifies them into the first real user-facing flow. Steps 7–9 build on top.

### Steps

- [x] **Step 1: Foundation** — Error types and config data structures that every module depends on.
  - Delivers: `src/error.rs` (AnvilError enum, 11 variants), `src/config/manifest.rs` (Manifest, Profile, Link, Hooks structs with `deny_unknown_fields`), `src/lib.rs`, all deps in `Cargo.toml`
  - Tests: 14 unit tests for manifest parsing
  - ~250 lines

- [x] **Step 2: CLI Skeleton** — Clap derive structs, stub command files, main dispatch loop.
  - Delivers: `src/cli/mod.rs`, `src/cli/{init,sync,apply,add,status,doctor}.rs` (stubs), `src/main.rs` dispatch
  - Depends on: Step 1
  - ~200 lines

- [x] **Step 3: UI Module** — Theme, spinner, prompt wrappers, and the full `UiContext` that controls `--yes`, `--quiet`, `--dry-run` behavior.
  - Delivers: `src/ui/mod.rs` (UiContext), `src/ui/theme.rs`, `src/ui/spinner.rs`, `src/ui/prompt.rs`, `src/ui/summary.rs`
  - Depends on: Step 1
  - ~300 lines

- [ ] **Step 4: Git Backend** — `GitBackend` trait abstracting git operations, with a `ShellGit` implementation that shells out to the git binary.
  - Delivers: `src/git/backend.rs` (trait + PullResult), `src/git/shell.rs` (ShellGit)
  - Depends on: Step 1
  - ~150 lines

- [ ] **Step 5: Linker** — Symlink creation/verification, copy-mode fallback, and path expansion via `dirs::home_dir()`.
  - Delivers: `src/linker/mod.rs`, `src/linker/symlink.rs`, `src/linker/copy.rs`, `ResolvedLink` struct
  - Depends on: Step 1
  - ~250 lines

- [ ] **Step 6: apply command** — The core user-facing flow: parse manifest, resolve links, run linker, handle conflicts with interactive prompts, print summary.
  - Delivers: full `src/cli/apply.rs`, hooks execution (`src/hooks/mod.rs`)
  - Depends on: Steps 2, 3, 4, 5
  - ~200 lines

- [ ] **Step 7: init command** — Prompt for repo URL, clone, parse manifest, run apply.
  - Delivers: full `src/cli/init.rs`
  - Depends on: Step 6
  - ~150 lines

- [ ] **Step 8: sync command** — Pull latest changes and re-apply links.
  - Delivers: full `src/cli/sync.rs`
  - Depends on: Step 6
  - ~80 lines

- [ ] **Step 9: Polish & edge cases** — Ctrl-C handling, non-TTY detection, clippy/fmt clean, full integration test pass.
  - Delivers: edge case hardening across all modules, integration tests in `tests/`
  - Depends on: Steps 7, 8
  - ~100 lines

---

## Phase 2 — Profiles & Machine Detection

Builds on the MVP to add multi-machine workflows and the remaining commands.

- [ ] Profile system with `extends` inheritance (circular dependency detection via HashSet)
- [ ] Multi-select profile picker in `init`
- [ ] Machine detection via `whoami::hostname()`
- [ ] `add` command with `toml_edit` for clean manifest updates (preserves comments/formatting)
- [ ] `status` command with table output
- [ ] Hooks with streamed stdout/stderr (prefixed with `│ `)

---

## Phase 3 — Polish & Distribution

Production hardening and public release.

- [ ] `doctor` command
- [ ] `--dry-run` support on all write commands
- [x] GitHub Actions: CI (fmt, clippy, tests on PRs)
- [ ] GitHub Actions: cross-platform release binaries (Linux x86_64, macOS x86_64/aarch64)
- [ ] `libgit2` backend (no git binary required)
- [ ] Shell completions via clap (bash, zsh, fish)
- [ ] Publish to crates.io

---

## Phase 4 — Secrets

Encrypted file support for sensitive configs.

- [ ] `age` encryption backend
- [ ] `gpg` encryption backend
- [ ] `anvil secrets add <file>` command

---

## Design Decisions

Key architectural choices that inform the implementation. See `anvil-architecture.md` for full details.

- **`UiContext` by `&` reference** — passed to every command function, no globals. Controls `--yes`, `--quiet`, and `--dry-run` centrally.
- **`deny_unknown_fields` on all serde structs** — fail loudly on `anvil.toml` typos rather than silently ignoring them.
- **`GitBackend` trait** — abstracts git operations behind a trait, enabling a future `libgit2` backend without changing command code.
- **No blanket `From<io::Error>`** — multiple error variants wrap `io::Error` with different semantics, so all conversions are explicit via `.map_err()`.
- **Tilde expansion via `dirs::home_dir()`** — never string-replace `~`. Always resolve through the platform-native home directory.
- **`toml_edit` for writes, `toml` for reads** — reading uses serde for ergonomics; writing preserves comments and formatting.
- **All terminal I/O through `ui/`** — commands never call `println!` directly. This makes `--quiet` and `--dry-run` work without conditionals in command code.
