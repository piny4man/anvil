# anvil — Architecture & Implementation Plan

> Dotfiles manager CLI written in Rust.
> Interactive, guided experience — like a good installer, not a silent Unix tool.
> Users store configs in any Git repo; anvil handles syncing, linking, and applying them across machines.

---

## Design Philosophy

anvil is **interactive by default**, silent only when piped or `--yes` is passed.
Every command has three layers:

```
1. Spinner / progress   →  indicatif (shows work happening)
2. Prompts              →  inquire   (asks when it needs to)
3. Result summary       →  console   (clear ✓ / ✗ output)
```

Flags for non-interactive use (CI, scripts):
- `--yes` / `-y`  — accept all defaults, no prompts
- `--dry-run`     — show what would happen, change nothing
- `--quiet`       — suppress all output except errors

---

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

### `anvil add ~/.zshrc`

```
  Moving ~/.zshrc into dotfiles repo
  ? Add to profile › base
  ? Link mode › ◉ symlink  ○ copy

  ✓ Moved   ~/.zshrc → ~/.dotfiles/.zshrc
  ✓ Linked  ~/.zshrc → ~/.dotfiles/.zshrc
  ✓ Updated anvil.toml

  Don't forget to commit: cd ~/.dotfiles && git add . && git commit
```

### `anvil doctor`

```
  Checking anvil setup...

  ✓ git             found (2.44.0)
  ✓ anvil.toml      valid
  ✓ Clone dir       ~/.dotfiles exists
  ✓ Symlinks        12/12 healthy
  ✗ waybar config   broken symlink → target missing
    Fix: anvil apply --force

  1 issue found.
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

---

## Repository Structure

```
anvil/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE                     # MIT
├── CHANGELOG.md
├── CONTRIBUTING.md
├── install.sh                  # curl-based bootstrap
│
├── src/
│   ├── main.rs                 # entrypoint: clap + run()
│   ├── lib.rs                  # public API (for testing)
│   │
│   ├── cli/
│   │   ├── mod.rs              # Cli struct, SubCommand enum (clap derive)
│   │   ├── init.rs             # `anvil init [url]`
│   │   ├── sync.rs             # `anvil sync`
│   │   ├── apply.rs            # `anvil apply`
│   │   ├── add.rs              # `anvil add <file>`
│   │   ├── status.rs           # `anvil status`
│   │   └── doctor.rs           # `anvil doctor`
│   │
│   ├── ui/                     # ALL terminal I/O lives here
│   │   ├── mod.rs              # re-exports, UiContext struct
│   │   ├── prompt.rs           # wrappers around inquire prompts
│   │   ├── spinner.rs          # wrappers around indicatif spinners
│   │   ├── theme.rs            # colors, symbols (✓ ✗ ⚠ →), style constants
│   │   └── summary.rs          # final result tables / summaries
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   ├── manifest.rs         # anvil.toml parsing (serde + deny_unknown_fields)
│   │   └── profile.rs          # profile resolution + extends inheritance
│   │
│   ├── git/
│   │   ├── mod.rs
│   │   ├── backend.rs          # trait GitBackend
│   │   └── shell.rs            # impl ShellGit (default)
│   │
│   ├── linker/
│   │   ├── mod.rs
│   │   ├── symlink.rs          # create/update/verify symlinks
│   │   └── copy.rs             # copy mode fallback
│   │
│   ├── hooks/
│   │   └── mod.rs              # run shell hooks, stream output
│   │
│   └── error.rs                # AnvilError (thiserror)
│
└── tests/
    ├── integration/
    │   ├── init_test.rs
    │   ├── apply_test.rs
    │   └── sync_test.rs
    └── fixtures/
        └── sample_dotfiles/    # minimal dotfiles repo for tests
```

---

## UiContext — Central I/O Controller

All commands receive a `UiContext`. This is the key to making `--yes` and `--quiet` work cleanly without sprinkling conditionals everywhere.

```rust
// ui/mod.rs

pub struct UiContext {
    pub yes: bool,     // --yes: skip all prompts, accept defaults
    pub quiet: bool,   // --quiet: suppress non-error output
    pub dry_run: bool, // --dry-run: no filesystem changes
}

impl UiContext {
    /// Returns user input OR the default value if --yes
    pub fn confirm(&self, msg: &str, default: bool) -> bool {
        if self.yes { return default; }
        prompt::confirm(msg, default)
    }

    pub fn select<T: Display>(&self, msg: &str, options: &[T], default: usize) -> usize {
        if self.yes { return default; }
        prompt::select(msg, options, default)
    }

    pub fn multi_select<T: Display>(&self, msg: &str, options: &[T]) -> Vec<usize> {
        if self.yes { return (0..options.len()).collect(); }
        prompt::multi_select(msg, options)
    }

    pub fn spinner(&self, msg: &str) -> Option<Spinner> {
        if self.quiet { return None; }
        Some(spinner::start(msg))
    }

    pub fn success(&self, msg: &str) {
        if !self.quiet { theme::print_success(msg); }
    }

    pub fn warn(&self, msg: &str) {
        theme::print_warn(msg); // warnings always shown
    }

    pub fn error(&self, msg: &str) {
        theme::print_error(msg); // errors always shown
    }
}
```

---

## UI Module Details

### ui/theme.rs — Symbols & Colors

```rust
// Uses `console` crate for cross-platform color support

pub const SYMBOL_OK:   &str = "✓";
pub const SYMBOL_ERR:  &str = "✗";
pub const SYMBOL_WARN: &str = "⚠";
pub const SYMBOL_ARROW:&str = "→";

pub fn print_success(msg: &str) {
    println!("{} {}", style(SYMBOL_OK).green().bold(), msg);
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}", style(SYMBOL_ERR).red().bold(), msg);
}

pub fn print_warn(msg: &str) {
    println!("{} {}", style(SYMBOL_WARN).yellow().bold(), msg);
}

pub fn print_header() {
    // ASCII art + version, shown only on init
}
```

### ui/spinner.rs — Progress Spinners

```rust
// Uses `indicatif` crate

pub struct Spinner(ProgressBar);

pub fn start(msg: &str) -> Spinner {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"])
            .template("{spinner:.cyan} {msg}").unwrap()
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    Spinner(pb)
}

impl Spinner {
    pub fn success(self, msg: &str) {
        self.0.finish_with_message(format!("{} {}", style("✓").green(), msg));
    }
    pub fn fail(self, msg: &str) {
        self.0.finish_with_message(format!("{} {}", style("✗").red(), msg));
    }
}
```

### ui/prompt.rs — Interactive Prompts

```rust
// Uses `inquire` crate

pub fn text(msg: &str, default: Option<&str>) -> Result<String> {
    let mut prompt = Text::new(msg);
    if let Some(d) = default { prompt = prompt.with_default(d); }
    Ok(prompt.prompt()?)
}

pub fn confirm(msg: &str, default: bool) -> bool {
    Confirm::new(msg).with_default(default).prompt().unwrap_or(default)
}

pub fn select<T: Display>(msg: &str, options: &[T], default: usize) -> usize {
    Select::new(msg, options.iter().collect())
        .with_starting_cursor(default)
        .prompt_skippable()
        // returns index of chosen option
}

pub fn multi_select<T: Display>(msg: &str, options: &[T]) -> Vec<usize> {
    MultiSelect::new(msg, options.iter().collect())
        .prompt()
        // returns indices of chosen options
}

pub enum ConflictAction { Overwrite, Skip, ShowDiff }

pub fn conflict_resolution(path: &Path) -> ConflictAction {
    Select::new(
        &format!("{} already exists. What to do?", path.display()),
        vec!["Overwrite", "Skip", "Show diff"]
    ).prompt()
    // map result to ConflictAction
}
```

---

## Command Flows (with UI)

### `anvil init [url] [--profile <n>] [-y]`

```
1. Show ASCII header (once, on init)
2. If no URL arg       → prompt::text("Dotfiles repo URL")
3. If no clone dir     → prompt::text("Clone into", default: "~/.dotfiles")
4. spinner "Cloning..."
   → git.clone(url, dest)
   → spinner.success / spinner.fail + early return
5. Parse anvil.toml from cloned repo
6. If profiles exist AND no --profile flag
   → prompt::multi_select("Available profiles")
7. If hostname not in [machines]
   → prompt::confirm("Set as default for this machine?")
   → write machine entry via toml_edit
8. Run apply (reuses apply flow)
9. Summary: "X linked, Y skipped, Z failed"
```

### `anvil apply [--profile <n>] [--dry-run] [-y]`

```
1. Parse anvil.toml
2. Resolve profile chain (detect circular extends → error)
3. Run hooks.before_apply (stream output with "│ " prefix)
4. For each Link:
   a. Expand ~ via dirs::home_dir()
   b. Create parent dirs (skip if --dry-run)
   c. Check dest:
      - Missing         → create symlink          → print ✓
      - Already ours    → skip                    → print ✓
      - Exists, foreign → prompt::conflict_resolution()
        · Overwrite     → backup to <dest>.bak, create symlink
        · Skip          → print ⚠, continue
        · Show diff     → run diff, then ask again
5. Run hooks.after_apply (stream output)
6. Print summary table
```

### `anvil sync [-y]`

```
1. spinner "Pulling latest changes..."
   → git.pull(dotfiles_dir)
   → spinner.success(pull_result.summary)
2. Re-run apply (shows what changed)
```

### `anvil add <file> [--profile <n>]`

```
1. Resolve absolute path of <file>
2. If no --profile → prompt::select from available profiles
3. prompt::select link mode ("symlink" / "copy")
4. Compute destination in repo (mirror directory structure)
5. If --dry-run   → print plan, exit
6. Move file into repo, create symlink back
7. Append link entry to anvil.toml via toml_edit (preserves comments)
8. Print ✓ steps + reminder to commit
```

### `anvil doctor`

```
1. Run checks sequentially, print ✓ or ✗ per check:
   - git binary present + version
   - anvil.toml parses without error
   - clone_dir exists
   - each symlink target exists
   - no obvious config files untracked by anvil (heuristic)
2. "All good!" or "N issues found."
   If issues → print suggested fix commands
```

---

## anvil.toml — Manifest Schema

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

---

## Core Data Structures

```rust
// config/manifest.rs

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub anvil: AnvilMeta,
    pub profiles: HashMap<String, Profile>,
    pub machines: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnvilMeta {
    pub version: String,
    pub default_profile: Option<String>,
    pub clone_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub extends: Option<String>,
    pub links: Vec<Link>,
    pub hooks: Option<Hooks>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub src: String,
    pub dest: String,
    pub copy: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Hooks {
    pub before_apply: Option<Vec<String>>,
    pub after_apply: Option<Vec<String>>,
}

// Resolved link after profile inheritance + path expansion
pub struct ResolvedLink {
    pub src: PathBuf,    // absolute path inside dotfiles repo
    pub dest: PathBuf,   // absolute path on the system
    pub copy: bool,
}
```

---

## Git Backend — Trait Design

```rust
// git/backend.rs

pub trait GitBackend {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()>;
    fn pull(&self, repo_dir: &Path) -> Result<PullResult>;
    fn status(&self, repo_dir: &Path) -> Result<String>;
}

pub struct PullResult {
    pub was_updated: bool,
    pub summary: String,  // "3 files changed" or "Already up to date"
}

// git/shell.rs — default impl, shells out to git binary

pub struct ShellGit;

impl GitBackend for ShellGit {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()> {
        let status = Command::new("git")
            .args(["clone", "--depth=1", url, dest.to_str().unwrap()])
            .stderr(Stdio::piped())
            .status()
            .map_err(AnvilError::GitNotFound)?;

        if !status.success() {
            return Err(AnvilError::GitCloneFailed(url.to_string()));
        }
        Ok(())
    }

    fn pull(&self, repo_dir: &Path) -> Result<PullResult> {
        let output = Command::new("git")
            .args(["pull", "--rebase"])
            .current_dir(repo_dir)
            .output()
            .map_err(AnvilError::GitNotFound)?;

        let summary = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PullResult {
            was_updated: !summary.contains("Already up to date"),
            summary,
        })
    }
}
```

> **Phase 3**: `Libgit2Backend` using the `git2` crate — same trait, no git binary required.

---

## Dependency Plan

```toml
[dependencies]
# CLI parsing
clap        = { version = "4",    features = ["derive", "env"] }

# Interactive UI
inquire     = "0.7"     # Text, Select, MultiSelect, Confirm prompts
indicatif   = "0.17"    # spinners and progress bars
console     = "0.15"    # colors, styles, is_term() detection

# Config
serde       = { version = "1", features = ["derive"] }
toml        = "0.8"     # manifest parsing (read)
toml_edit   = "0.22"    # manifest editing (write without losing formatting)

# Utilities
thiserror   = "1"       # error types
dirs        = "5"       # cross-platform home dir expansion
whoami      = "1"       # hostname for machine profile detection

[dev-dependencies]
tempfile    = "3"       # safe temp dirs in integration tests
assert_cmd  = "2"       # CLI integration testing
predicates  = "3"       # output assertions
```

---

## CLI Struct (clap)

```rust
// cli/mod.rs

#[derive(Parser)]
#[command(name = "anvil", about = "Dotfiles manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Accept all defaults, skip prompts
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Show what would happen without making changes
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Bootstrap dotfiles on a new machine
    Init {
        url: Option<String>,
        #[arg(short, long)]
        profile: Vec<String>,
    },
    /// Pull latest changes and re-apply
    Sync,
    /// Apply dotfiles to the system
    Apply {
        #[arg(short, long)]
        profile: Vec<String>,
    },
    /// Adopt an existing file into the dotfiles repo
    Add {
        file: PathBuf,
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Show current link status
    Status,
    /// Check for common issues
    Doctor,
}
```

---

## main.rs

```rust
fn main() {
    let cli = Cli::parse();

    let ctx = UiContext {
        yes: cli.yes,
        quiet: cli.quiet,
        dry_run: cli.dry_run,
    };

    // Auto-enable quiet when output is piped
    let ctx = if !console::Term::stdout().is_term() {
        UiContext { quiet: true, ..ctx }
    } else {
        ctx
    };

    let result = match cli.command.unwrap_or(Command::Status) {
        Command::Init { url, profile }    => init::run(url, profile, &ctx),
        Command::Sync                     => sync::run(&ctx),
        Command::Apply { profile }        => apply::run(profile, &ctx),
        Command::Add { file, profile }    => add::run(file, profile, &ctx),
        Command::Status                   => status::run(&ctx),
        Command::Doctor                   => doctor::run(&ctx),
    };

    if let Err(e) = result {
        ctx.error(&e.to_string());
        std::process::exit(1);
    }
}
```

---

## Implementation Phases

### Phase 1 — Interactive MVP
- [ ] `ui/` module: `UiContext`, `theme`, `spinner`, `prompt`
- [ ] `init` with URL/dir prompts + clone spinner
- [ ] `apply` with conflict resolution prompt (overwrite / skip / diff)
- [ ] Basic `anvil.toml` parsing (links only, no profiles yet)
- [ ] `sync` command
- [ ] `--yes` flag works everywhere

### Phase 2 — Profiles & machine detection
- [ ] Profile system with `extends` (circular detection via HashSet)
- [ ] `multi_select` profile picker in `init`
- [ ] Machine detection via `whoami::hostname()`
- [ ] `add` command (with `toml_edit` for clean manifest updates)
- [ ] `status` command with table output
- [ ] Hooks with streamed output (prefixed with `│ `)

### Phase 3 — Polish & distribution
- [ ] `doctor` command
- [ ] `--dry-run` on all write commands
- [ ] GitHub Actions: CI + cross-platform release binaries
- [ ] `libgit2` backend (no git binary required)
- [ ] Shell completions via clap (zsh, fish, bash)
- [ ] Publish to crates.io

### Phase 4 — Secrets
- [ ] `age` encryption backend
- [ ] `gpg` encryption backend
- [ ] `anvil secrets add <file>`

---

## GitHub Actions

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check

  release:
    if: startsWith(github.ref, 'refs/tags/')
    strategy:
      matrix:
        include:
          - { target: x86_64-unknown-linux-gnu,  os: ubuntu-latest }
          - { target: x86_64-apple-darwin,        os: macos-latest }
          - { target: aarch64-apple-darwin,       os: macos-latest }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: "${{ matrix.target }}" }
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: softprops/action-gh-release@v1
        with:
          files: target/${{ matrix.target }}/release/anvil
```

---

## Bootstrap install.sh

```bash
#!/usr/bin/env bash
# curl -sSf https://raw.githubusercontent.com/youruser/anvil/main/install.sh | sh
set -e

REPO="youruser/anvil"
BINARY="anvil"
INSTALL_DIR="${HOME}/.local/bin"

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)        ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

LATEST=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep tag_name | cut -d'"' -f4)
URL="https://github.com/${REPO}/releases/download/${LATEST}/${BINARY}-${OS}-${ARCH}"

mkdir -p "$INSTALL_DIR"
curl -sSfL "$URL" -o "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

echo "✓ anvil ${LATEST} installed to ${INSTALL_DIR}/${BINARY}"
echo "  Make sure ${INSTALL_DIR} is in your \$PATH"
echo ""
echo "  Get started: anvil init https://github.com/you/dotfiles"
```

---

## Notes for Claude Code

- The `ui/` module is the **only** place that touches the terminal — commands never call `println!` directly
- `UiContext` is passed by reference to every command function — no globals
- `--yes` makes all prompts return their default silently — essential for scripting/CI
- `console::Term::stdout().is_term()` — auto-enable `--quiet` when not a TTY (piped output)
- Use `toml_edit` (not `toml`) when writing back to `anvil.toml` — preserves comments and formatting
- Profile `extends` resolution must detect cycles — use a `HashSet<String>` of visited names
- Path expansion: always use `dirs::home_dir()`, never `str::replace("~", ...)`
- Symlink creation: check if dest is already a symlink pointing to the correct src before touching it
- Hooks stream stdout/stderr with a `│ ` prefix — don't capture it, pipe it live
- Integration tests use `tempfile::tempdir()` — never touch the real home directory
- `inquire` prompts handle non-TTY gracefully — cover this path in tests
- `#[serde(deny_unknown_fields)]` on all config structs — fail loudly on typos in anvil.toml
