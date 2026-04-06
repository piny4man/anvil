# PRD-001: Phase 1 Interactive MVP — Complete Implementation

## Problem Statement

anvil is a dotfiles manager CLI that currently has its foundation in place (error types, config parsing, CLI skeleton, UI module) but cannot actually do anything yet. The six command stubs (`init`, `sync`, `apply`, `add`, `status`, `doctor`) are empty shells. There is no git integration, no symlink management, no hook execution, and no local state persistence. A user who installs anvil today gets a binary that parses flags and exits silently.

The core value proposition — clone a dotfiles repo, link config files to their correct locations, and keep them in sync — requires implementing the remaining Phase 1 steps (4–9) from the roadmap.

## Solution

Implement the complete Phase 1 MVP: the `init`, `sync`, and `apply` commands with full interactive flows. This means building four new modules (git backend, linker, hooks, local config), wiring them into the three command stubs, and hardening the result with comprehensive tests.

After this work, a user can:
1. Run `anvil init <url>` to clone their dotfiles repo and apply links
2. Run `anvil sync` to pull changes and re-apply
3. Run `anvil apply` to re-link manually at any time
4. Get interactive conflict resolution when files already exist
5. Have hooks run automatically before/after applying

## User Stories

1. As a new user, I want to run `anvil init https://github.com/me/dotfiles` so that my dotfiles repo is cloned and all my config files are symlinked into place on a fresh machine.
2. As a new user, I want anvil to prompt me for a repo URL if I run `anvil init` without arguments, so that I don't have to memorize the CLI syntax.
3. As a new user, I want anvil to prompt me for a clone directory with a sensible default (`~/.dotfiles`), so that I can customize it or just press Enter.
4. As a new user, I want to see a spinner while the repo is cloning, so that I know something is happening.
5. As a new user, I want anvil to show a clear summary after init (e.g., "4 linked, 1 skipped"), so that I know what happened.
6. As a new user, I want anvil to write `~/.config/anvil.toml` after a successful init, so that subsequent commands know where my dotfiles live and which profiles are active.
7. As a returning user, I want to run `anvil sync` from anywhere and have it pull the latest changes and re-apply links, so that I stay up to date without manually navigating to my dotfiles directory.
8. As a returning user, I want `anvil sync` to show me what changed (e.g., "3 files changed" or "Already up to date"), so that I have visibility into updates.
9. As a user, I want to run `anvil apply` to re-apply all links without pulling, so that I can fix broken links or re-apply after manual edits to `anvil.toml`.
10. As a user, I want anvil to skip symlinks that already point to the correct target, so that re-running apply is fast and idempotent.
11. As a user, I want anvil to prompt me when a destination file already exists and is not managed by anvil, offering Overwrite / Skip / Show diff, so that I don't lose local changes.
12. As a user choosing "Show diff", I want to see an inline diff between my existing file and the repo version, so that I can make an informed decision about overwriting.
13. As a user choosing "Overwrite", I want anvil to back up my existing file to `<file>.bak` before replacing it, so that I have a safety net.
14. As a user, I want `before_apply` hooks from `anvil.toml` to run before any links are created, so that prerequisites (e.g., creating directories, installing packages) are handled first.
15. As a user, I want `after_apply` hooks to run after all links are created, so that post-setup tasks (e.g., reloading shell config) happen automatically.
16. As a user, I want to see hook output streamed live with a `│ ` prefix, so that I can distinguish hook output from anvil's own messages.
17. As a user, I want hook failures to be reported clearly with the exit code and command that failed, so that I can debug issues.
18. As a user with `copy: true` on a link, I want anvil to copy the file instead of symlinking, so that I can handle files that don't work as symlinks (e.g., some apps rewrite the file in place).
19. As a scripter, I want `--yes` to accept all defaults without prompting (skip conflicts, accept default clone dir), so that I can use anvil in CI or automation.
20. As a scripter, I want `--quiet` to suppress all non-error output, so that anvil works cleanly in pipelines.
21. As a scripter, I want `--dry-run` to show what would happen without making changes, so that I can preview the effect of apply/init/sync safely.
22. As a user piping anvil output, I want anvil to auto-detect non-TTY and behave as if `--quiet` was passed, so that spinners and prompts don't corrupt my pipeline.
23. As a user, I want Ctrl-C to cleanly abort any operation without leaving partial state (half-created symlinks, incomplete clones), so that I can safely cancel at any point.
24. As a user, I want clear error messages when git is not installed, the repo URL is invalid, the clone directory already exists, or `anvil.toml` is missing from the repo.
25. As a user, I want `anvil apply --profile base` to apply only links from a specific profile, so that I can selectively apply parts of my config.
26. As a user on a machine where `~/.config/anvil.toml` doesn't exist and no `--dir` is provided, I want anvil to check `~/.dotfiles` as a default before erroring, so that the common case works without configuration.
27. As a contributor, I want comprehensive integration tests covering init/apply/sync flows and edge cases, so that regressions are caught automatically.

## Implementation Decisions

### New Modules

**git/ — Git Backend**
- `GitBackend` trait with three methods: `clone_repo(url, dest)`, `pull(repo_dir)`, `status(repo_dir)`
- `ShellGit` implementation that shells out to the `git` binary
- Clone always uses `--depth=1` (shallow clone) for fast setup
- Pull does NOT pass `--rebase` — respects the user's own git config for pull strategy
- `PullResult` struct contains `was_updated: bool` and `summary: String`
- Error variants already exist in `AnvilError`: `GitNotFound`, `GitCloneFailed`, `GitPullFailed`

**linker/ — Symlink & Copy Management**
- `ResolvedLink` struct: absolute `src` (inside dotfiles repo), absolute `dest` (on the system), `copy: bool`
- Path expansion always via `dirs::home_dir()`, never string replacement on `~`
- Symlink creation: check if dest is already a correct symlink before touching it (idempotent)
- Copy mode triggered only by explicit `copy: true` in the manifest link entry
- Conflict detection: if dest exists and is not our symlink, return a conflict status for the UI layer to handle
- Parent directory creation: `create_dir_all` for dest parent path before linking
- `--dry-run` support: return what would happen without making filesystem changes

**hooks/ — Hook Execution**
- Executes shell commands listed in `before_apply` and `after_apply` arrays from the manifest
- Working directory is the dotfiles repo root
- stdout/stderr are streamed live, prefixed with `│ ` for visual separation
- Hook failure (non-zero exit) stops execution and reports the failing command + exit code
- `--dry-run` prints which hooks would run without executing them

**config/local.rs — Local State**
- Manages `~/.config/anvil.toml` (not to be confused with the manifest `anvil.toml` inside the dotfiles repo)
- Stores `clone_dir` (path to dotfiles repo) and `profiles` (list of active profile names)
- Written after successful `init`, read by `sync` and `apply` to discover the repo
- Uses `toml_edit` for writes to stay consistent with the project's write strategy
- Uses `toml`/`serde` for reads

**Repo discovery order:** read `~/.config/anvil.toml` → fall back to `~/.dotfiles` → error with "run `anvil init`"

### Modified Modules

**cli/apply.rs — Apply Command**
1. Discover dotfiles repo (local config → `~/.dotfiles` fallback)
2. Parse `anvil.toml` manifest from repo
3. Resolve profile to use (from `--profile` flag, local config, or manifest default)
4. Run `before_apply` hooks
5. For each link: expand paths → check dest → create symlink/copy or handle conflict
6. Run `after_apply` hooks
7. Print summary via `ApplySummary`

**cli/init.rs — Init Command**
1. Print ASCII header
2. Prompt for repo URL if not provided as arg
3. Prompt for clone directory (default: `~/.dotfiles`)
4. Clone repo via `GitBackend` (shallow)
5. Parse `anvil.toml` from cloned repo
6. If `--profile` not specified, use manifest's `default_profile` or prompt
7. Run the full apply flow
8. Write `~/.config/anvil.toml` with clone_dir + selected profiles

**cli/sync.rs — Sync Command**
1. Read `~/.config/anvil.toml` to find repo dir (fall back to `~/.dotfiles`)
2. Pull via `GitBackend`
3. Show pull result (spinner → success/fail message)
4. Re-run apply flow

### Conflict Resolution Flow
- When dest exists and is not managed by anvil: prompt Overwrite / Skip / Show diff
- "Show diff": use the `similar` crate for inline diff rendering (built-in, no external `diff` binary dependency)
- "Overwrite": backup existing file to `<dest>.bak`, then create symlink/copy
- "Skip": log as skipped, continue to next link
- With `--yes`: default to Skip (safe, non-destructive)

### Ctrl-C / Signal Handling
- Register a handler that sets an atomic flag
- Check the flag between link operations — abort cleanly with partial summary
- Clone operation: if interrupted, remove the partially cloned directory

### Non-TTY Detection
- Already implemented in `main.rs`: auto-sets `quiet = true` when `!Term::stdout().is_term()`
- Prompts in `UiContext` already return defaults when `yes = true`
- Ensure hooks still stream output even in quiet mode (they represent user scripts, not anvil UI)

## Testing Decisions

### Testing philosophy
- Test external behavior, not implementation details
- Integration tests exercise the CLI binary end-to-end via `assert_cmd`
- Unit tests verify module contracts in isolation
- All tests use `tempfile::tempdir()` — never touch the real home directory or filesystem
- Mock the git backend for tests that don't need real clones (use the `GitBackend` trait)
- Prior art: 14 existing unit tests in `config/manifest.rs` establish the pattern

### Modules with unit tests

**git/ (git backend)**
- `ShellGit` constructs correct command arguments
- `PullResult` parsing from git output
- Error mapping for missing git binary, failed clone, failed pull

**linker/ (symlink & copy)**
- Creates symlink correctly in a temp directory
- Detects existing correct symlink (idempotent — no-op)
- Detects conflict (dest exists, not our symlink)
- Copy mode creates a file copy instead of symlink
- Parent directory creation
- Tilde expansion resolves correctly
- `--dry-run` returns plan without filesystem side effects

**hooks/ (hook execution)**
- Successful hook execution returns ok
- Failed hook (non-zero exit) returns error with exit code
- Hook working directory is set to dotfiles repo
- Hook stdout/stderr captured and prefixed
- `--dry-run` does not execute hooks

**config/local.rs (local state)**
- Write and read round-trip for `~/.config/anvil.toml`
- Missing config file returns None/default
- Tilde expansion in clone_dir

**cli/ commands (command logic)**
- `apply`: links created for a simple manifest, conflicts detected, hooks called in order, summary counts correct
- `init`: clone triggered, manifest parsed, local config written, apply called
- `sync`: local config read, pull triggered, apply called
- All commands respect `--dry-run`, `--yes`, `--quiet` flags

### Integration tests

**init flow**
- `anvil init <url>` with a local bare git repo as fixture → clones, applies links, writes local config
- `anvil init` without URL in `--yes` mode → errors cleanly (no URL to default to)
- Init into existing directory → appropriate error

**apply flow**
- Apply with valid manifest → all symlinks created
- Apply with conflicts → prompts shown (or skipped with `--yes`)
- Apply with `copy: true` links → files copied
- Apply with hooks → hooks executed in correct order
- Apply idempotent — running twice produces same result
- Apply with `--dry-run` → no filesystem changes

**sync flow**
- Sync pulls and re-applies
- Sync with no local config and no `~/.dotfiles` → helpful error

**Edge cases**
- Broken symlinks (target deleted after linking)
- Permission errors (read-only dest directory)
- Non-TTY mode (piped output, no prompts)
- Ctrl-C during apply (partial state cleanup)
- Missing git binary → clear error message
- Missing `anvil.toml` in cloned repo → clear error
- Empty profiles (zero links) → no-op with success message

## Out of Scope

- **Profile inheritance (`extends`)** — deferred to Phase 2. MVP profiles are flat (no resolution chain).
- **Multi-select profile picker** — deferred to Phase 2. MVP uses `--profile` flag or manifest default.
- **Machine detection via hostname** — deferred to Phase 2. No automatic profile selection by machine.
- **`add` command** — deferred to Phase 2. Requires `toml_edit` manifest writes.
- **`status` command** — deferred to Phase 2. Requires iterating links and checking state.
- **`doctor` command** — deferred to Phase 3.
- **`--dry-run` on init/sync** — partially supported (apply respects it), but clone/pull still execute. Full dry-run for all write operations is Phase 3.
- **`libgit2` backend** — Phase 3. MVP shells out to git.
- **Shell completions** — Phase 3.
- **Secrets / encryption** — Phase 4.
- **Cross-platform release binaries** — Phase 3.
- **crates.io publishing** — Phase 3.

## Further Notes

- The `similar` crate will be added as a new dependency for built-in diff rendering in conflict resolution. This avoids requiring `diff` to be installed on the system.
- The `GitBackend` trait enables swapping in `libgit2` later (Phase 3) without modifying any command code.
- Local config at `~/.config/anvil.toml` is intentionally simple and separate from the dotfiles manifest. It's anvil's own state, not the user's config.
- Hook output streaming with `│ ` prefix should use `BufReader` line-by-line reading on the child process's stdout/stderr to avoid buffering the entire output.
- The `ApplySummary` struct already exists in `ui/summary.rs` — command implementations will use it directly.
- Estimated scope: ~930 new lines of Rust across the new modules, plus ~500-700 lines of tests.
