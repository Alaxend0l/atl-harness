# Agent task: runner registry, harness tooling, documentation

Standing task for coding agents (opencode on strata, Claude Code). Work
through it in order, one item per branch/commit. Read `AGENTS.md`,
`README.md`, `docs/architecture.md` and `docs/setup.md` first.

## Rules for this task
- `AGENTS.md` hard rules apply. Nothing here touches upstream ATL.
- **Useful beats many.** Every tool must be something a person running the
  harness would reach for at least weekly. If you can't name the moment
  it's used, don't write it.
- Every tool: `--help`/usage text, an entry in `docs/usage.md`, and a unit
  test if it's Rust. Shell scripts: `set -euo pipefail`, pass `shellcheck`.
- Prefer a new `atlh` subcommand over a script when it reads the DB or
  config. Scripts are for things that orchestrate machines (ssh, rsync).
- Don't invent facts in docs. If you haven't run it, say "untested". Mark
  anything you inferred rather than observed.
- Before committing: `cargo fmt && cargo clippy --all-targets && cargo test`.
- Update `docs/roadmap.md` when an item is done, and `docs/decisions.md`
  when you choose between real alternatives.

## 1. Runner registry (do this first)

Today one runner is hard-wired through `ATLH_REMOTE`/`ATLH_REMOTE_BOX`.
More are coming (an M4 Mac mini running an aarch64 Linux VM is next).
Adding a machine should be: add one entry, run one command.

### Design (follow it unless you find a concrete problem; then record why in decisions.md)
- `runners.toml` at the repo root, strictly typed like `config.toml`:
  ```toml
  [runners.bazzite]
  ssh = "atl-box"            # ssh destination / alias
  distrobox = "atl"          # optional: wrap commands in `distrobox enter`
  dir = "atl-harness"        # remote checkout, relative to ~
  arch = "x86_64"            # uname -m; selects ABI defaults
  config = "config.toml"     # harness config to use on this runner
  jobs = 4                   # default --jobs

  [runners.mini]
  ssh = "atl-mini-vm"
  arch = "aarch64"
  config = "config.aarch64.toml"
  jobs = 3
  ```
- `scripts/remote.sh` takes a runner name: `scripts/remote.sh <runner>
  sync|atlh|pull|shell|doctor`. With `ATLH_RUNNER` set, the name can be
  omitted. Keep `ATLH_REMOTE`-only use working for one release, with a
  deprecation note.
- `pull` must not mix runners: pull into `work/<runner>/`.
- Run ids should record their runner. Add a `runner` column to `runs`
  (migration for existing DBs), set from `--runner` or `ATLH_RUNNER`.
  `compare` must refuse to compare runs from different runners unless
  `--cross-runner` is passed. x86_64 vs aarch64 results are not
  comparable.
- Per-arch config: `config.aarch64.toml` with
  `allowed_abis = ["arm64-v8a"]`. Avoid duplicating the whole file if a
  small include/override mechanism is simple; if not, duplicate and say so.

### `doctor`
`scripts/remote.sh <runner> doctor` checks a runner and prints a pass/fail
table with a fix hint per line:
ssh reachable · distrobox exists · `uname -m` matches `arch` · cargo ·
Xvfb · ImageMagick (`convert`) · `android-translation-layer` on PATH ·
ART installed (`pkg-config art-standalone`) · disk free in `dir` ·
clock skew vs workstation (warn > 5 s) · repo synced (compare a hash of
`src/`).

### `bootstrap` (stretch)
`scripts/bootstrap-runner.sh <runner>`: idempotent setup of a fresh
Fedora/Fedora-distrobox runner, following `docs/setup.md` (packages, Rust,
wolfSSL, bionic_translation, art_standalone, libOpenSLES, ATL). Each step
skips itself if already done. Log to `~/src/logs/bootstrap-<step>.log` on
the runner. Long steps (art_standalone) run detached; the script reports
how to watch them. Do not run `rpm-ostree` or touch the host system of an
immutable distro.

### Docs
`docs/runners.md`: what a runner is, how to add one (checklist from ssh
key to first green `doctor`), the per-arch caveats (16K pages on Asahi;
no 32-bit ARM on Apple Silicon; see decisions D10).

## 2. Harness tools (pick by usefulness, in roughly this order)

1. `atlh show RUN PKG`: outcome, signatures, log path, screenshot path,
   the log's exception chain. The first thing anyone runs after `rank`.
2. `atlh history PKG`: outcome and top signature across all runs.
3. `atlh signature SIG_ID|TEXT`: apps hit, first/last seen run, sample
   stack trace.
4. `atlh runs`: list run ids with date, runner, app count, outcome
   histogram.
5. `atlh report RUN --html out.html`: static page with the rank table and
   a thumbnail grid of screenshots grouped by outcome. Single file,
   no external assets.
6. `atlh blind-spots RUN`: broken apps with no signature, with the last 30
   lines of each log, to help write classifier rules.
7. Calibration helper for `blank_stddev_threshold`: list scores of apps
   per outcome so a human can pick the threshold from real data.
8. `atlh retry RUN --outcome CRASH`: rerun only apps with a given outcome
   (flake checking).

Each needs tests that use an in-memory SQLite and no ATL/Xvfb.

## 3. Documentation

- `docs/troubleshooting.md`: every failure seen so far and its fix. Start
  with the ones in git history: temp HOME missing `.local/share`,
  `GDK_DEBUG=gl-egl` vs `GDK_DISABLE=glx`, stale remote builds from rsync
  mtimes and clock skew, `work_dir` resolved relative to `--config`.
- `docs/signatures.md`: a worked example per signature kind, taken from
  real `work/runs` logs, showing log excerpt → signature.
- `docs/atl-notes.md`: what we learned about ATL's behaviour (launch
  activity selection, data dir layout, title bar, env vars from
  upstream `doc/Envs.md`) with a source file reference for each claim.
- Keep `README.md` short; link, don't duplicate.

## Done means
Each item merged with tests green, its docs written, and the roadmap
updated. Report at the end: what you built, what you skipped and why.
