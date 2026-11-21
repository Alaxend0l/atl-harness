# Notes for coding agents

Read by opencode (`AGENTS.md`) and Claude Code (via `CLAUDE.md`).

## What this is
`atlh`: a Rust harness that runs F-Droid apps under a **hard fork** of
Android Translation Layer (ATL), classifies failures into signatures, ranks
them, gates on regressions, and generates prompts for an automated fix loop.
Start with `HANDOFF.md` (current state), then `README.md` and `docs/roadmap.md`.

## Hard rules
- **Nothing goes to upstream ATL.** Upstream (postmarketOS developers) has a
  no-AI policy. Never open PRs or issues upstream, never suggest
  upstreaming, never add upstream as a push target. See `docs/decisions.md`
  D1.
- Don't fake fixes in the ATL fork (catch-alls, `return null`, empty
  bodies). The harness exists to measure real compatibility.

## Environment
- A Linux **workstation** with this clone, and **runners** on the LAN
  listed in `runners.toml`: an x86_64 box (`bazzite`,
  in a distrobox), an aarch64 Lima VM on a Mac mini (`mini`) and a OnePlus 12
  phone (`oneplus12`, bwrap rootfs in `/home/phone` only; follow the
  fedora-atomic-phone safety rules). Mini and phone run apps only, never
  agents. ATL, Xvfb and the corpus live on the runners.
  Use `scripts/rig` for everything remote (`docs/rig.md`); don't hand-roll
  ssh/rsync lines. Build ATL changes with `rig build --worktree
  ~/Develop/android-translation-layer` (reports parsed errors), then gate with
  `rig atlh run` + `rig atlh compare`. Run anything longer than a few
  minutes as a detached job (`--detach`, then `rig job wait`); rig commands
  always end with a verdict (exit 124 timeout, 125 stalled, 126 unreachable,
  see docs/rig.md) instead of hanging. See `docs/setup.md`.
- `work/` (corpus, logs, screenshots, sqlite) is git-ignored.

## Working on the code
- New files start with `SPDX-License-Identifier: AGPL-3.0-or-later` (LICENSING.md).
- Before committing: `cargo fmt && cargo clippy --all-targets && cargo test`
  and `python3 -m unittest discover -s scripts/tests`.
- Unit tests must not need Xvfb, ATL or network.
- Config is strictly typed (`deny_unknown_fields`): add new keys to the
  structs in `src/config.rs` and to `config.toml` and `docs/usage.md`.
- Only the main thread writes to SQLite (see `docs/architecture.md`).
- Record direction changes in `docs/decisions.md`; update
  `docs/roadmap.md` status when finishing a step.
