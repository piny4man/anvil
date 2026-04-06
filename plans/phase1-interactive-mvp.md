# Plan: Phase 1 Interactive MVP

> Source PRD: `prd/001-phase1-interactive-mvp.md`

## Architectural decisions

Durable decisions that apply across all phases:

- **Local config path**: `~/.config/anvil.toml` stores `clone_dir` and `profiles`. Written by `init`, read by `sync`/`apply`.
- **Repo discovery order**: read local config -> fall back to `~/.dotfiles` -> error with "run `anvil init`"
- **Git backend**: `GitBackend` trait with `ShellGit` impl. Clone uses `--depth=1`. Pull respects user's git config (no `--rebase`).
- **Linker model**: `ResolvedLink` struct with absolute `src`, absolute `dest`, `copy: bool`. Path expansion via `dirs::home_dir()`.
- **Conflict strategy**: Overwrite (with `.bak` backup) / Skip / Show diff. `--yes` defaults to Skip.
- **Hook execution**: Shell commands from manifest, working dir = repo root, streamed output with `| ` prefix.
- **UiContext threading**: All commands receive `&UiContext`. Flags (`--yes`, `--quiet`, `--dry-run`) are handled centrally.
- **New dependency**: `similar` crate for inline diff rendering (no external `diff` binary).
- **Testing**: Unit tests per module, integration tests via `assert_cmd` + `tempfile::tempdir()`. Mock git via `GitBackend` trait.
- **ROADMAP sync**: When a phase is completed, check off the corresponding `ROADMAP.md` step(s). Mapping below.

| Plan Phase | ROADMAP Step(s) |
|------------|-----------------|
| Phase 1 (Apply) | Step 5 (Linker) |
| Phase 2 (Init) | Step 4 (Git Backend), Step 7 (init command) |
| Phase 3 (Sync) | Step 8 (sync command) |
| Phase 4 (Conflict Resolution) | Step 6 (apply command) — partial |
| Phase 5 (Hooks) | Step 6 (apply command) — completes it |
| Phase 6 (Copy/dry-run/scripting) | — (no direct ROADMAP step) |
| Phase 7 (Signal/errors) | Step 9 (Polish & edge cases) — partial |
| Phase 8 (Integration tests) | Step 9 (Polish & edge cases) — completes it |

---

## Phase 1: Apply — Link files from an existing repo

**User stories**: 9, 10, 25, 26

### What to build

The narrowest useful vertical slice: given a dotfiles repo already on disk, anvil reads the manifest, resolves a profile's links, and creates symlinks. This requires three new pieces wired together end-to-end:

1. **`config/local.rs`** — Read and write `~/.config/anvil.toml` (the local state file that tells anvil where the dotfiles repo lives and which profiles are active). Serde for reads, `toml_edit` for writes.
2. **`linker/`** — Resolve links from a profile into absolute paths, create symlinks with parent directory creation, and detect when a symlink already points to the correct target (idempotent no-op). No conflict handling yet — if a non-symlink file exists at the destination, report it as a conflict and skip.
3. **`cli/apply.rs`** — Wire it together: discover repo via local config or `~/.dotfiles` fallback, parse manifest, resolve the target profile (from `--profile` flag, local config, or manifest default), create symlinks, print summary via `ApplySummary`.

After this phase, a user who manually clones their dotfiles repo and writes a local config can run `anvil apply` and get symlinks created.

### Acceptance criteria

- [x] `config/local.rs` can write and round-trip read `clone_dir` and `profiles`
- [x] `config/local.rs` returns `None` when the local config file doesn't exist
- [x] Linker creates a symlink from `dest -> src` in a temp directory
- [x] Linker detects an existing correct symlink and skips it (idempotent)
- [x] Linker creates parent directories for dest paths that don't exist yet
- [x] Linker reports conflict when dest exists and is not the expected symlink
- [x] `anvil apply` discovers repo from local config, falls back to `~/.dotfiles`, errors with guidance if neither exists
- [x] `anvil apply --profile base` applies only links from the named profile
- [x] `ApplySummary` correctly counts linked/skipped/failed and prints via `UiContext`
- [x] Unit tests for local config read/write, linker symlink creation, idempotency, and conflict detection
- [x] `--quiet` suppresses non-error output from apply
- [x] ROADMAP.md updated (Step 5 checked off, Step 6 noted as partial)
- [x] `rust-pro` agent review
- [x] `documentation-expert` agent review

---

## Phase 2: Init — Clone a repo and apply

**User stories**: 1, 2, 3, 4, 5, 6

### What to build

The first-run experience: `anvil init <url>` clones a dotfiles repo, runs the apply flow from Phase 1, and writes local config so subsequent commands know where to find things.

1. **`git/`** — `GitBackend` trait with `clone_repo(url, dest)` method. `ShellGit` impl shells out to `git clone --depth=1`. Checks for git binary existence first.
2. **`cli/init.rs`** — Print ASCII header, prompt for repo URL if not provided as arg, prompt for clone directory (default `~/.dotfiles`), clone via git backend with spinner, parse `anvil.toml` from cloned repo (error clearly if missing), resolve profile (from `--profile` flag or manifest default), run the apply flow, write local config.

After this phase, a new user can run `anvil init https://github.com/me/dotfiles` and get a fully linked setup.

### Acceptance criteria

- [x] `GitBackend` trait defined with `clone_repo` method
- [x] `ShellGit` constructs correct `git clone --depth=1` command
- [x] Missing git binary produces `AnvilError::GitNotFound` with a clear message
- [x] Failed clone produces `AnvilError::GitCloneFailed` with the URL
- [x] `anvil init <url>` clones repo, applies links, writes local config
- [x] `anvil init` without URL prompts interactively; errors in `--yes` mode (no URL to default to)
- [x] Clone directory prompt defaults to `~/.dotfiles`
- [x] Spinner shown during clone, replaced by success/failure message
- [x] Missing `anvil.toml` in cloned repo produces `AnvilError::ManifestNotFound`
- [x] After successful init, `~/.config/anvil.toml` exists with correct `clone_dir` and `profiles`
- [x] Unit tests for git command construction, error mapping
- [x] Integration test: `anvil init` with a local git repo fixture

---

## Phase 3: Sync — Pull and re-apply

**User stories**: 7, 8

### What to build

The daily workflow: `anvil sync` pulls the latest changes from the remote and re-applies links.

1. **`git/`** — Add `pull(repo_dir)` method to `GitBackend`. `ShellGit` runs `git pull` (no `--rebase`). Returns `PullResult` with `was_updated: bool` and `summary: String`.
2. **`cli/sync.rs`** — Read local config to find repo dir (fall back to `~/.dotfiles`), pull via git backend with spinner, show result ("Already up to date" or change summary), re-run apply flow.

After this phase, the core daily loop works: init once, sync whenever.

### Acceptance criteria

- [ ] `GitBackend` trait extended with `pull` method returning `PullResult`
- [ ] `ShellGit::pull` runs `git pull` in the repo directory
- [ ] Pull result correctly reports whether changes were fetched
- [ ] Failed pull produces `AnvilError::GitPullFailed`
- [ ] `anvil sync` reads local config, pulls, and re-applies
- [ ] `anvil sync` with no local config and no `~/.dotfiles` produces a helpful error
- [ ] Spinner shown during pull, replaced by result message
- [ ] Unit tests for pull command construction, result parsing
- [ ] Integration test: sync pulls and re-applies links

---

## Phase 4: Conflict resolution

**User stories**: 11, 12, 13

### What to build

When `apply` encounters an existing file at a destination that isn't managed by anvil, it now prompts the user instead of just skipping. The interactive flow offers three choices: Overwrite, Skip, or Show diff.

1. **Linker conflict handling** — When a conflict is detected, return it to the command layer for UI-driven resolution instead of auto-skipping.
2. **Diff rendering** — Add the `similar` crate. When the user chooses "Show diff", render an inline text diff between the existing file and the repo version, then re-prompt.
3. **Backup on overwrite** — When the user chooses "Overwrite", copy the existing file to `<dest>.bak` before creating the symlink/copy.
4. **`--yes` behavior** — Default to Skip (safe, non-destructive) when prompts are suppressed.

The `conflict_resolution` prompt method already exists on `UiContext` — this phase wires it into the apply loop.

### Acceptance criteria

- [ ] Existing non-managed file at dest triggers Overwrite/Skip/Show diff prompt
- [ ] "Show diff" displays inline diff using `similar` and re-prompts
- [ ] "Overwrite" backs up existing file to `<dest>.bak` then creates link
- [ ] "Skip" logs the file as skipped and continues
- [ ] `--yes` mode defaults to Skip without prompting
- [ ] `ApplySummary` reflects overwritten and skipped counts accurately
- [ ] Unit tests for backup creation, diff output, conflict flow with each choice
- [ ] Integration test: apply with conflicting files exercises prompt paths

---

## Phase 5: Hook execution

**User stories**: 14, 15, 16, 17

### What to build

Shell hooks from the manifest's `before_apply` and `after_apply` arrays run at the right points in the apply flow.

1. **`hooks/`** — Execute shell commands with working directory set to the dotfiles repo root. Stream stdout/stderr live, line-by-line via `BufReader`, prefixed with `| ` for visual separation. On non-zero exit, stop execution and return an error with the failing command and exit code.
2. **Wire into apply** — Run `before_apply` hooks before any links are created, `after_apply` hooks after all links are created (in `apply`, `init`, and `sync` flows).
3. **`--dry-run` support** — Print which hooks would run without executing them.

### Acceptance criteria

- [ ] `before_apply` hooks execute before link creation
- [ ] `after_apply` hooks execute after link creation
- [ ] Hook output streams live with `| ` prefix
- [ ] Hook failure (non-zero exit) stops execution and reports command + exit code
- [ ] Hook working directory is the dotfiles repo root
- [ ] `--dry-run` prints hook names without executing them
- [ ] Hooks still stream output even in `--quiet` mode (they are user scripts, not anvil UI)
- [ ] Unit tests for hook execution, failure reporting, working directory
- [ ] Integration test: apply with hooks verifies execution order

---

## Phase 6: Copy mode, dry-run, and scripting hardening

**User stories**: 18, 19, 20, 21, 22

### What to build

Polish the apply flow for edge cases and automation use.

1. **Copy mode** — When a link entry has `copy: true`, copy the file instead of creating a symlink. Conflict resolution applies the same way.
2. **`--dry-run` threading** — Ensure dry-run is respected throughout: linker reports what it would do without filesystem changes, hooks print but don't execute (Phase 5), init/sync still clone/pull but apply is dry.
3. **Non-TTY auto-quiet** — Already partially implemented in `main.rs`. Verify prompts return defaults and spinners are suppressed when piped.
4. **`--yes` completeness** — Verify all prompts in init (URL, dir) and apply (conflicts) short-circuit correctly.

### Acceptance criteria

- [ ] `copy: true` link entries produce file copies instead of symlinks
- [ ] Copied files are byte-identical to source
- [ ] `--dry-run` on apply shows what would happen without filesystem changes
- [ ] `--dry-run` on init/sync still clones/pulls but apply portion is dry
- [ ] `--quiet` suppresses all non-error output (spinners, summaries, success messages)
- [ ] Non-TTY detection auto-sets quiet behavior
- [ ] `--yes` skips all prompts with sensible defaults
- [ ] Unit tests for copy mode, dry-run link planning
- [ ] Integration test: piped output produces no interactive elements

---

## Phase 7: Signal handling and error hardening

**User stories**: 23, 24

### What to build

Make anvil resilient to interruption and clear about failures.

1. **Ctrl-C handling** — Register a signal handler that sets an atomic flag. Check the flag between link operations; abort cleanly with a partial summary. During clone, remove the partially cloned directory on interrupt.
2. **Error messages** — Ensure clear, actionable messages for: git not installed, invalid repo URL, clone directory already exists, `anvil.toml` missing from repo, permission errors on dest directories, broken symlinks.

### Acceptance criteria

- [ ] Ctrl-C during apply aborts cleanly with partial summary (no half-created state)
- [ ] Ctrl-C during clone removes the partially cloned directory
- [ ] Missing git binary error tells the user to install git
- [ ] Invalid repo URL error includes the URL that failed
- [ ] Clone into existing directory error suggests a different path or `anvil sync`
- [ ] Missing `anvil.toml` error tells the user what file is expected and where
- [ ] Permission errors on dest directory are reported with the path
- [ ] Unit tests for signal flag checking, error message content

---

## Phase 8: Integration test hardening

**User stories**: 27

### What to build

A comprehensive integration test suite that exercises all flows end-to-end and covers edge cases not tested in earlier phases. Earlier phases include happy-path integration tests; this phase adds cross-cutting and adversarial scenarios.

1. **Init edge cases** — Init into existing directory, init without URL in `--yes` mode, init with repo missing `anvil.toml`.
2. **Apply edge cases** — Broken symlinks (target deleted), empty profiles (zero links), re-apply idempotency (run twice, same result).
3. **Sync edge cases** — Sync with no local config and no `~/.dotfiles`.
4. **Cross-cutting** — Non-TTY mode (piped output, no prompts), `--dry-run` produces no filesystem side effects, `--quiet` suppresses output.
5. **Error paths** — Missing git binary, permission errors on read-only directories.

### Acceptance criteria

- [ ] Init into existing directory produces appropriate error
- [ ] Init without URL in `--yes` mode errors cleanly
- [ ] Init with repo missing `anvil.toml` errors clearly
- [ ] Apply with broken symlinks handles gracefully
- [ ] Apply with empty profile succeeds with no-op summary
- [ ] Apply is idempotent (running twice produces same filesystem state)
- [ ] Sync with no config and no `~/.dotfiles` produces helpful error
- [ ] Non-TTY mode suppresses prompts and spinners
- [ ] `--dry-run` across all commands produces no filesystem changes
- [ ] Missing git binary produces clear error message
- [ ] All tests use `tempfile::tempdir()` — no real home directory touched
