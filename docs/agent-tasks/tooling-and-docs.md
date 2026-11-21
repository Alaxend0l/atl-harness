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

## 1. Runner registry: mostly done

`scripts/rig` + `runners.toml` exist (see `docs/rig.md`, decisions D12):
add, list, doctor, sync, atlh, sh, pull, jobs, bootstrap, rebuild-atl,
smoke, up/down. Remaining, in order:
- `rig census RUN --all`: start the same `atlh run` on every runner as
  jobs, wait for all, pull. Results stay per runner.
- `rig report --all`: side-by-side outcome counts per runner for a run id
  (read the pulled `work/remote/*/atlh.sqlite`).
- Store the ATL commit (`git -C <atl_src> rev-parse HEAD`) with each run
  (new `runs` column or a `run_meta` table), so results are traceable to a
  build. Needs a migration for existing DBs.
- `rig doctor`: also check that `[atl].command`'s binary matches the
  checkout's HEAD (stale install after a fork change).

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
