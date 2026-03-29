# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**anvil** is a dotfiles manager CLI written in Rust. It provides an interactive, guided experience for syncing, linking, and applying config files across machines. Users store configs in any Git repo; anvil handles the rest.

The project is in early development (scaffold stage). See `anvil-architecture.md` for the full implementation plan and design spec.

## Build & Development Commands

```bash
cargo build              # build
cargo run                # run
cargo test               # run all tests
cargo test <test_name>   # run a single test
cargo clippy -- -D warnings  # lint (treat warnings as errors)
cargo fmt --check        # check formatting
cargo fmt                # auto-format
```

Rust edition: **2024**

## Architecture

### Module Layout (planned)

- `src/main.rs` — entrypoint: clap parsing + dispatch to command modules
- `src/cli/` — one file per subcommand (`init`, `sync`, `apply`, `add`, `status`, `doctor`)
- `src/ui/` — **all terminal I/O lives here** — commands never call `println!` directly
- `src/config/` — `anvil.toml` manifest parsing and profile resolution
- `src/git/` — `GitBackend` trait with `ShellGit` impl (shells out to `git`)
- `src/linker/` — symlink creation/verification and copy-mode fallback
- `src/hooks/` — shell hook execution with streamed output
- `src/error.rs` — `AnvilError` via `thiserror`

### Key Design Patterns

- **`UiContext`** is passed by `&` reference to every command function (no globals). It controls `--yes`, `--quiet`, and `--dry-run` behavior centrally so commands don't sprinkle conditionals.
- **`GitBackend` trait** abstracts git operations, enabling a future `libgit2` backend without changing command code.
- **`anvil.toml`** is the manifest file users place in their dotfiles repo. Parsed with `serde` + `deny_unknown_fields` on all config structs to fail loudly on typos. Written back with `toml_edit` to preserve comments/formatting.
- **Profile inheritance**: profiles can `extends` another profile. Resolution must detect cycles via `HashSet<String>`.

### Important Conventions

- Path expansion: always use `dirs::home_dir()`, never string replacement on `~`
- Auto-quiet when piped: detect non-TTY via `console::Term::stdout().is_term()`
- Symlinks: check if dest already points to correct src before touching it
- Hooks stream stdout/stderr live with `│ ` prefix — don't capture, pipe it
- Integration tests use `tempfile::tempdir()` — never touch the real home directory

## Agent Routing
- After implementing any new Rust function, module, or struct → delegate to the `documentation-expert` agent
- When asked to "add docs" or "document" anything → always use the `documentation-expert` agent
- Never write documentation inline in the main session
- After implementing any new Rust change, delegate to `rust-pro` agent
- On every cli ui plan check with `cli-ui-designer` agent
- On every cli ui change delegate to `cli-ui-designer` agent
