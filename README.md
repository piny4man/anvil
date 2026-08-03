# anvil

Interactive-by-default **dotfiles manager** for Linux (Arch-first).  
Clone a Git repo, link configs, optionally install packages, decrypt age secrets, and audit personal hardening posture.

```text
  ▗▄▖ █▄ █ █ █ ██▄ █
 ▐▌ ▐▌█ ▀█ ▀▄▀ █▄█ █▄▄
 dotfiles manager v0.1.0
```

> **Status: 0.1 pre-release** — usable for personal dogfooding. The `anvil.toml` schema may still change before 1.0. See [ROADMAP.md](ROADMAP.md) and [Production readiness](#production-readiness).

---

## Why anvil

| Need | How anvil helps |
|------|-----------------|
| New machine setup | `anvil init <git-url>` clones, scaffolds config if needed, applies |
| Day-to-day sync | `anvil sync` pulls + re-applies |
| Adopt existing files | `anvil add ~/.zshrc` moves into the repo and links back |
| Preview safely | `anvil apply --dry-run` prints a plan; nothing is written |
| Recover mistakes | Overwrites write a **backup journal**; `anvil undo` restores |
| Arch packages | Declarative `packages.pacman` / `packages.aur` |
| Secrets | `decrypt = "age"` on link entries |
| Hardening | sysctl / sshd / ufw / home checks via `doctor` and `apply --harden` |

Unlike Stow (silent) or pure git wrappers, anvil is **guided in a TTY** and **scriptable** with `-y` / `--dry-run` / `--force`.

---

## Install

### From source (current)

```bash
git clone https://github.com/piny4man/anvil
cd anvil
cargo install --path .
```

Requires **Rust 1.85+** (edition 2024) and a `git` binary on `PATH`.

```bash
anvil --version
anvil --help
```

Pre-built release binaries are not published yet.

---

## Quick start

### A. Empty or non-anvil dots repo (first time)

```bash
anvil init https://github.com/you/dotfiles
# If anvil.toml is missing → offered a starter scaffold (Yes by default)
anvil add ~/.zshrc
anvil add ~/.config/nvim
# commit anvil.toml + moved files in the dots repo
anvil apply
anvil status
anvil doctor
```

### B. Repo already has `anvil.toml`

```bash
anvil init https://github.com/you/dotfiles
# select profiles when prompted, or:
anvil init https://github.com/you/dotfiles -p base -p hyprland -y
anvil sync          # later: pull + re-apply
anvil apply --dry-run
anvil apply -y      # non-interactive; conflicts are skipped unless --force
```

### C. Non-interactive / CI-style

```bash
anvil apply -y --dry-run
anvil apply -y --force          # overwrite conflicts (backs up first)
anvil apply -y --packages       # also install missing pacman/AUR pkgs
anvil apply -y --harden         # run posture checks (+ enforce if configured)
```

---

## Two config layers

| Location | Shared? | Purpose |
|----------|---------|---------|
| `<repo>/anvil.toml` | Yes (Git) | Profiles, links, hooks, packages, harden |
| `~/.config/anvil/config.toml` | No (local) | `repo_path`, active `profiles`, `age_identity` |
| `~/.local/state/anvil/backups/` | No (local) | Backup journals for `anvil undo` |

### Local config example

```toml
# ~/.config/anvil/config.toml
repo_path = "/home/you/.dotfiles"
profiles = ["base", "hyprland"]
age_identity = "~/.config/age/key.txt"
```

Written automatically by `anvil init`. Edit by hand if you move the clone.

---

## Manifest (`anvil.toml`)

Full annotated sample: [`examples/anvil.toml`](examples/anvil.toml).

```toml
[anvil]
version = "1"
default_profile = "base"
# clone_dir = "~/.dotfiles"   # used as init default only

[profiles.base]
links = [
  { src = ".zshrc",       dest = "~/.zshrc" },
  { src = ".gitconfig",   dest = "~/.gitconfig" },
  { src = ".config/nvim", dest = "~/.config/nvim" },
  # optional: copy instead of symlink
  # { src = "mimeapps.list", dest = "~/.config/mimeapps.list", copy = true },
  # optional: age-encrypted secret
  # { src = "secrets/npmrc.age", dest = "~/.npmrc", mode = "600", decrypt = "age" },
]
hooks.after_apply = ["scripts/post-apply.sh"]

packages.pacman = ["git", "ufw", "age"]
# packages.aur = ["some-aur-package"]

[profiles.base.harden]
mode = "check"   # or "enforce" (sysctl drop-in via sudo)
sysctl = [{ key = "kernel.kptr_restrict", value = "2" }]
ssh = { password_auth = false, root_login = false }
firewall = { backend = "ufw", default = "deny", allow = ["22/tcp"] }

[profiles.work]
extends = "base"
links = [
  { src = "work/.gitconfig", dest = "~/.gitconfig" },  # later wins on same dest
]

[machines]
"framework-arch" = ["base", "hyprland"]
```

### Field reference

#### `[anvil]`

| Field | Required | Description |
|-------|----------|-------------|
| `version` | yes | Schema version (`"1"`) |
| `default_profile` | no | Used when no CLI `-p`, local profiles, or machine match |
| `clone_dir` | no | Default clone path for init (`~/.dotfiles`) |

#### `[profiles.<name>]`

| Field | Description |
|-------|-------------|
| `extends` | Parent profile name (chain; cycles error) |
| `links` | File link entries |
| `hooks.before_apply` / `after_apply` | Repo-relative scripts (no `..`, no absolute paths) |
| `packages.pacman` / `packages.aur` | Package names (Arch) |
| `harden` | Posture checks / optional enforce |

**Merge rules:** ordered profile lists (CLI / machines / local) are primary. `extends` prepends parents. **Later links win** on the same `dest`. Hooks and packages concatenate (packages deduped).

#### Link entry

| Field | Required | Description |
|-------|----------|-------------|
| `src` | yes | Path relative to the repo root |
| `dest` | yes | System path (`~` and `~/...` expanded) |
| `copy` | no | `true` = copy file/dir instead of symlink |
| `mode` | no | Octal mode string, e.g. `"600"` (also inferred under `~/.ssh/`) |
| `decrypt` | no | `"age"` — decrypt `src` to `dest` with the `age` CLI |

#### `[machines]`

Hostname → list of profiles. Matched via system hostname when no `-p` / local profiles override.

Unknown keys in the manifest are **rejected** (`deny_unknown_fields`) so typos fail loudly.

---

## Commands

| Command | What it does |
|---------|----------------|
| `anvil init [url] [--dir] [-p …]` | Clone (or reuse), scaffold `anvil.toml` if missing, write local config, apply if links exist |
| `anvil sync [--pull-only]` | `git pull --rebase --autostash`, then apply |
| `anvil apply [-p …] [--packages] [--harden]` | Build a plan, link files, optional packages/harden |
| `anvil add <path> [-p profile]` | Move into repo, link back, append to `anvil.toml` |
| `anvil status [-p …]` | Linked / conflict / missing, packages, harden |
| `anvil doctor` | git, manifest, symlink health, secret scan, harden |
| `anvil undo` | Restore from latest backup journal |

Default command when none is given: **`status`**.

### Global flags

| Flag | Meaning |
|------|---------|
| `-y`, `--yes` | Accept prompt defaults; **does not** force overwrites |
| `--force` | Overwrite conflicts (after backup). Required for destructive apply |
| `--dry-run` | Show plan only |
| `-q`, `--quiet` | Errors only (also when stdout is not a TTY) |

### Safety model

1. **`--yes` is not `--force`.** Under `-y`, file conflicts are **skipped** unless `--force`.
2. **Backups before overwrite.** Journal under `~/.local/state/anvil/backups/<id>/`.
3. **Hooks** only run repo-relative scripts; absolute paths and `..` are rejected.
4. **Harden enforce** only when `harden.mode = "enforce"` and you pass `--harden` (sysctl drop-in may use `sudo`).
5. Prefer **`anvil apply --dry-run`** before the first apply on a real home directory.

---

## Workflows

### Day-to-day

```bash
anvil sync                 # pull + apply
anvil status
anvil doctor
```

### Adopt a new config

```bash
anvil add ~/.config/kitty
cd "$(anvil …)"   # or: cd ~/.dotfiles
git add -A && git commit -m "add kitty"
```

### Secrets with age

```bash
# encrypt into the repo (example)
age -r <recipient> -o secrets/npmrc.age ~/.npmrc

# anvil.toml
# { src = "secrets/npmrc.age", dest = "~/.npmrc", mode = "600", decrypt = "age" }

# ~/.config/anvil/config.toml
# age_identity = "~/.config/age/key.txt"

anvil apply -y
```

Requires the [`age`](https://github.com/FiloSottile/age) CLI on `PATH`.

### Packages (Arch)

```toml
[profiles.base]
packages.pacman = ["ufw", "fail2ban", "age"]
packages.aur = []   # needs paru or yay
```

```bash
anvil apply --packages -y
```

### Hardening

```bash
anvil doctor
anvil apply --harden --dry-run
anvil apply --harden -y          # check; enforce only if mode = "enforce"
```

---

## What it looks like

### `anvil init` (existing anvil.toml)

```text
  ▗▄▖ █▄ █ █ █ ██▄ █
 ▐▌ ▐▌█ ▀█ ▀▄▀ █▄█ █▄▄
 dotfiles manager v0.1.0

? Clone into › ~/.dotfiles
  ✓ Cloned into /home/you/.dotfiles
? Available profiles: (space to select)
  ❯ base
    hyprland
  ✓ Local config written to ~/.config/anvil/config.toml
  Applying profile: base + hyprland
  ✓ ~/.zshrc → symlinked
  Done! 4 linked, 0 skipped
```

### Empty repo scaffold

```text
  ✓ Cloned into /home/you/.dotfiles
  ⚠ No anvil.toml found in this repository.
? Create a starter anvil.toml so you can bootstrap this machine? (Y/n)
  ✓ Wrote starter anvil.toml
  ⚠ No links defined yet — repo is ready for bootstrap.
  Next steps:
    1. cd ~/.dotfiles
    2. anvil add ~/.zshrc
    3. git add anvil.toml && git commit
```

### `anvil apply --dry-run`

```text
  Profile: base
  Repo:    /home/you/.dotfiles
  FILES     3 to link, 1 ok, 0 conflict, 0 broken
  [link] .zshrc → ~/.zshrc
  ✓ Dry-run complete (no changes made)
```

---

## Production readiness

Honest checklist for **0.1**:

| Area | Ready? | Notes |
|------|--------|--------|
| Core link/apply/sync/init | Yes | Integration-tested dry-run + symlink apply |
| Docs for basic use | Yes | This README + `examples/anvil.toml` |
| Manifest schema stability | **No (pre-1.0)** | May gain fields; unknown keys already rejected |
| Multi-distro packages | No | Arch pacman/AUR only |
| macOS / Windows | Partial | Unix symlinks; packages/harden are Linux-oriented |
| Prebuilt binaries / install.sh | No | Source install only |
| Shell completions | No | Planned |
| Secrets without `age` CLI | No | Shells out to `age` |
| Concurrent apply / multi-user | No | Single-user personal tool |
| Formal security audit | No | Review hooks and `--force` before untrusted repos |

**Recommendation:** ship and dogfood as **personal pre-release**; do not call it production-stable for untrusted third-party dots repos without reading hooks and package lists first.

---

## Development

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Architecture notes: [anvil-architecture.md](anvil-architecture.md) (may lag code slightly; prefer README for user-facing behavior).

---

## License

[MIT](LICENSE)
