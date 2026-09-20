# anvil

Interactive-by-default **dotfiles manager** for Linux (Arch-first).  
Clone a Git repo, link configs, optionally install packages, decrypt age secrets, and audit personal hardening posture.

```text
                                   ███  ████
                                  ▒▒▒  ▒▒███
  ██████   ████████   █████ █████ ████  ▒███
 ▒▒▒▒▒███ ▒▒███▒▒███ ▒▒███ ▒▒███ ▒▒███  ▒███
  ███████  ▒███ ▒███  ▒███  ▒███  ▒███  ▒███
 ███▒▒███  ▒███ ▒███  ▒▒███ ███   ▒███  ▒███
▒▒████████ ████ █████  ▒▒█████    █████ █████
 ▒▒▒▒▒▒▒▒ ▒▒▒▒ ▒▒▒▒▒    ▒▒▒▒▒    ▒▒▒▒▒ ▒▒▒▒▒

  forge your machine · v0.1.0
```

> **Status: 0.1 pre-release** — usable for personal dogfooding. The `anvil.toml` schema may still change before 1.0. See [ROADMAP.md](ROADMAP.md) and [Production readiness](#production-readiness).

---

## Why anvil

| Need | How anvil helps |
|------|-----------------|
| New machine setup | `anvil init <git-url>` full-clones, scaffolds if needed, shows a plan, then apply |
| Day-to-day sync | `anvil sync` pulls + re-applies |
| Adopt existing files | `anvil add ~/.zshrc` journals, copies into the repo, links back |
| Preview safely | `anvil apply --dry-run` prints a plan; nothing is written |
| Recover mistakes | Durable **backup journals**; `anvil undo` / `undo --list` / `undo --id` |
| Arch packages | Official repos via pacman; AUR via a **machine-local** helper |
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
anvil init https://github.com/you/dotfiles --no-apply
# If anvil.toml is missing → offered a starter scaffold (Yes by default)
anvil add ~/.zshrc
anvil add ~/.config/nvim
# commit anvil.toml + moved files in the dots repo
anvil apply --dry-run
anvil apply -y
anvil apply --packages -y          # official + AUR (helper from local config)
anvil apply --harden               # checks; enforce only if harden.mode = "enforce"
anvil status
anvil doctor
```

### B. Repo already has `anvil.toml`

```bash
anvil init https://github.com/you/dotfiles
# select profiles when prompted, or:
anvil init https://github.com/you/dotfiles -p base -p hyprland -y
# reuse a checkout only if origin URL matches; otherwise error
anvil sync          # later: pull + re-apply
anvil apply --dry-run
anvil apply -y      # non-interactive; conflicts are skipped unless --force
```

### C. Non-interactive / CI-style

```bash
anvil init <url> --dir ~/.dotfiles --no-apply -y
anvil apply -y --dry-run
anvil apply -y --force                    # overwrite conflicts (journal first)
anvil apply -y --packages                 # pacman + AUR helper (auto/paru/yay)
anvil apply -y --packages --aur-helper anzen
anvil apply --harden                      # checks; confirm before enforce
anvil undo --list
anvil undo                                # latest active journal
```

---

## Two config layers

| Location | Shared? | Purpose |
|----------|---------|---------|
| `<repo>/anvil.toml` | Yes (Git) | Profiles, links, hooks, packages, harden |
| `~/.config/anvil/config.toml` | No (local) | `repo_path`, `profiles`, `age_identity`, `aur_helper` |
| `~/.local/state/anvil/backups/` | No (local) | Backup journals for `anvil undo` |

### Local config example

```toml
# ~/.config/anvil/config.toml
repo_path = "/home/you/.dotfiles"
profiles = ["base", "hyprland"]
age_identity = "~/.config/age/key.txt"
# Machine-local AUR helper. Not in anvil.toml.
# auto (default) = first of paru, yay on PATH — never auto-picks anzen
aur_helper = "auto"   # or "paru" | "yay" | "anzen" | "/abs/path"
```

Written automatically by `anvil init`. Edit by hand if you move the clone. `aur_helper` is **not** a valid key in `anvil.toml`.

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
| `anvil init [url] [--dir] [-p …] [--no-apply]` | Full clone (or reuse if origin matches), scaffold if missing, write local config, show plan, confirm then apply |
| `anvil sync [--pull-only]` | `git pull --rebase --autostash`, then apply |
| `anvil apply [-p …] [--packages] [--harden] [--aur-helper]` | Plan, link files, optional packages/harden |
| `anvil add <path> [-p profile]` | Journal dest, copy into repo, link back, append `anvil.toml` |
| `anvil status [-p …]` | Linked / pending / conflict / broken, packages, harden |
| `anvil doctor` | git, manifest, symlink health, AUR helper, secrets, harden |
| `anvil undo [--list] [--id]` | Restore a backup journal (latest active, or a specific id) |

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
2. **Journal before mutate.** Each overwrite writes `journal.json` immediately under `~/.local/state/anvil/backups/<id>/`. Empty applies do not leave a journal. Restored journals are marked `restored` and skipped by `undo` unless you pass `--id`.
3. **Hooks cannot be undone.** Dry-run lists them; interactive apply confirms; `-y` still prints the warning. Repo-relative only (no `..`, no absolute paths).
4. **Packages this run** are recorded on the same journal. `anvil undo` restores files, then asks (default **No**) to `sudo pacman -R` those names. `--yes` does **not** uninstall; `--force` does. Never `anzen remove`.
5. **Harden enforce** only when `harden.mode = "enforce"` and you pass `--harden`. The sysctl drop-in is journaled before write. SSH/firewall stay check-only.
6. Prefer **`anvil apply --dry-run`** before the first apply on a real home directory.

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
cd ~/.dotfiles
git add -A && git commit -m "add kitty"
```

`add` refuses a path that is already a symlink into this repo. The dest is journaled before the move, so `anvil undo` can restore it if linking fails.

### Undo a bad apply

```bash
anvil undo --list          # id, status, entry count
anvil undo                 # latest active journal (confirm)
anvil undo --id <id>       # a specific journal
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

`packages.pacman` always uses `pacman`. `packages.aur` uses a **machine-local** helper — never stored in `anvil.toml`.

| Helper | How it is chosen | Install argv |
|--------|------------------|--------------|
| `auto` (default) | First of `paru`, `yay` on `PATH`. **Never** auto-picks `anzen`. | `paru`/`yay -S --needed` |
| `paru` / `yay` | CLI `--aur-helper` or `aur_helper` in local config | `-S --needed` [ `--noconfirm` if `-y` ] |
| `anzen` | Only when you choose it (GPL-3; anvil execs the binary, never links it) | `anzen install` [ `--noconfirm` if `-y` ]. Never `--skipreview` / `--ask`. Review stays interactive. |

```toml
[profiles.base]
packages.pacman = ["ufw", "fail2ban", "age"]
packages.aur = ["some-aur-package"]
```

```bash
# ~/.config/anvil/config.toml
# aur_helper = "paru"

anvil apply --packages -y
anvil apply --packages --aur-helper anzen   # review cannot be skipped
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
                                   ███  ████
                                  ▒▒▒  ▒▒███
  ██████   ████████   █████ █████ ████  ▒███
 ▒▒▒▒▒███ ▒▒███▒▒███ ▒▒███ ▒▒███ ▒▒███  ▒███
  ███████  ▒███ ▒███  ▒███  ▒███  ▒███  ▒███
 ███▒▒███  ▒███ ▒███  ▒▒███ ███   ▒███  ▒███
▒▒████████ ████ █████  ▒▒█████    █████ █████
 ▒▒▒▒▒▒▒▒ ▒▒▒▒ ▒▒▒▒▒    ▒▒▒▒▒    ▒▒▒▒▒ ▒▒▒▒▒

  forge your machine · v0.1.0

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
  AUR helper: paru
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
