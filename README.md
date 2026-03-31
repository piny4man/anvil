# anvil

A dotfiles manager that guides you. Clone, link, sync — interactively or hands-free.

> **Pre-release** — anvil is under active development. The manifest format is stabilizing but not yet guaranteed stable. See [ROADMAP.md](ROADMAP.md) for progress.

## What It Looks Like

### `anvil init`

```
  ▗▄▖ █▄ █ █ █ ██▄ █
 ▐▌ ▐▌█ ▀█ ▀▄▀ █▄█ █▄▄
 dotfiles manager

? Dotfiles repo URL › https://github.com/user/dotfiles
? Clone into › ~/.dotfiles

  ⠸ Cloning repository...
  ✓ Cloned into ~/.dotfiles

? Available profiles: (use space to select)
  ❯ ◉ base
    ○ hyprland
    ○ work

? Set as default for this machine (framework-arch)? (Y/n)

  Applying profile: base + hyprland

  ✓ ~/.zshrc               → symlinked
  ✓ ~/.gitconfig           → symlinked
  ✓ ~/.config/nvim         → symlinked
  ✓ ~/.config/hyprland     → symlinked
  ⚠ ~/.config/waybar       → already exists
    ? Overwrite? (y/N/diff)

  ✓ Running hook: scripts/install_packages.sh

  Done! 4 linked, 1 skipped.
  Run `anvil status` to see the current state.
```

### `anvil sync`

```
  ⠸ Pulling latest changes...
  ✓ Already up to date. (or: 3 files changed)

  ⠸ Re-applying links...
  ✓ All links up to date.
```

### `anvil status`

```
  Profile: base + hyprland  (machine: framework-arch)
  Repo:    git@github.com:user/dotfiles  (clean, up to date)

  LINKED
  ✓ ~/.zshrc              → ~/.dotfiles/.zshrc
  ✓ ~/.gitconfig          → ~/.dotfiles/.gitconfig
  ✓ ~/.config/nvim        → ~/.dotfiles/.config/nvim
  ✓ ~/.config/hyprland    → ~/.dotfiles/.config/hyprland

  BROKEN
  ✗ ~/.config/waybar      → target missing

  UNTRACKED (files in repo not in any profile)
  ? .config/kitty
```

## Features

- Interactive prompts with sane defaults — skip everything with `--yes`
- Symlink and copy modes for each config file
- Profile system with inheritance (`extends`) for machine-specific overlays
- Auto-select profiles per hostname via `[machines]` table
- Pre/post-apply hooks for running install scripts
- Conflict detection — overwrite, skip, or diff before touching existing files
- `--dry-run` to preview changes without touching the filesystem
- Works with any Git-hosted dotfiles repo

## Quick Start

1. **Install anvil** (see [Installation](#installation))

2. **Add `anvil.toml` to your dotfiles repo:**

   ```toml
   [anvil]
   version = "1"

   [profiles.base]
   links = [
     { src = ".zshrc",       dest = "~/.zshrc" },
     { src = ".gitconfig",   dest = "~/.gitconfig" },
     { src = ".config/nvim", dest = "~/.config/nvim" },
   ]
   ```

3. **Bootstrap on a new machine:**

   ```bash
   anvil init https://github.com/you/dotfiles
   ```

4. **Pull updates later:**

   ```bash
   anvil sync
   ```

5. **Adopt existing config files:**

   ```bash
   anvil add ~/.config/kitty
   ```

## Configuration

anvil is configured via an `anvil.toml` file in the root of your dotfiles repo.

```toml
[anvil]
version = "1"
default_profile = "base"
# clone_dir = "~/.dotfiles"    # optional, defaults to ~/.dotfiles

[profiles.base]
links = [
  { src = ".zshrc",           dest = "~/.zshrc" },
  { src = ".gitconfig",       dest = "~/.gitconfig" },
  { src = ".config/nvim",     dest = "~/.config/nvim" },
]
hooks.after_apply = ["scripts/install_packages.sh"]

[profiles.work]
extends = "base"
links = [
  { src = "work/.gitconfig",  dest = "~/.gitconfig" },   # overrides base
  { src = "work/.ssh/config", dest = "~/.ssh/config" },
]

[profiles.hyprland]
extends = "base"
links = [
  { src = ".config/hyprland", dest = "~/.config/hyprland" },
  { src = ".config/waybar",   dest = "~/.config/waybar" },
]

[machines]
"framework-arch" = ["base", "hyprland"]
"work-macbook"   = ["base", "work"]
```

### Reference

#### `[anvil]`

| Field | Required | Description |
|-------|----------|-------------|
| `version` | yes | Manifest schema version (currently `"1"`) |
| `default_profile` | no | Profile applied when none is specified |
| `clone_dir` | no | Where to clone the repo (default: `~/.dotfiles`) |

#### `[profiles.<name>]`

| Field | Required | Description |
|-------|----------|-------------|
| `extends` | no | Inherit links and hooks from another profile |
| `links` | no | List of file link entries (default: `[]`) |
| `hooks` | no | Scripts to run before/after applying |

#### Link entries

| Field | Required | Description |
|-------|----------|-------------|
| `src` | yes | Path relative to the dotfiles repo root |
| `dest` | yes | Destination path on the system (`~` is expanded) |
| `copy` | no | `true` to copy instead of symlink (default: symlink) |

#### `hooks`

| Field | Description |
|-------|-------------|
| `before_apply` | List of scripts to run before linking |
| `after_apply` | List of scripts to run after linking |

#### `[machines]`

Maps hostnames to profile lists. When anvil detects a matching hostname, it applies those profiles automatically.

## Commands

| Command | Description |
|---------|-------------|
| `anvil init [url]` | Clone a dotfiles repo and apply profiles |
| `anvil sync` | Pull latest changes and re-apply links |
| `anvil apply` | Apply dotfiles links to the system |
| `anvil add <file>` | Adopt an existing config file into the repo |
| `anvil status` | Show current link status |
| `anvil doctor` | Check for common setup issues |

### Global flags

| Flag | Short | Description |
|------|-------|-------------|
| `--yes` | `-y` | Accept all defaults, skip prompts |
| `--dry-run` | | Show what would happen, change nothing |
| `--quiet` | `-q` | Suppress output except errors |

## Installation

### From source

```bash
git clone https://github.com/OWNER/anvil
cd anvil
cargo install --path .
```

Requires Rust 2024 edition (rustc 1.85+).

Pre-built binaries and a `curl | sh` installer will be available once the tool reaches MVP.

## Building from Source

```bash
cargo build                    # build
cargo test                     # run all tests
cargo clippy -- -D warnings    # lint (warnings as errors)
cargo fmt --check              # check formatting
```

## Project Status

anvil is in early development, working through **Phase 1** of the roadmap.

| Step | Status |
|------|--------|
| 1. Foundation (error types, config structs) | Done |
| 2. CLI Skeleton (clap dispatch, stub commands) | Done |
| 3. UI Module (UiContext, prompts, spinners) | Done |
| 4. Git Backend (GitBackend trait, ShellGit) | — |
| 5. Linker (symlinks, copy fallback) | — |
| 6–9. Commands & polish | — |

See [ROADMAP.md](ROADMAP.md) for the full plan.

## License

MIT
