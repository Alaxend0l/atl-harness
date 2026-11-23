# Notes for coding agents

Read by opencode (`AGENTS.md`) and Claude Code (via `CLAUDE.md`).

## What this is
`atlh`: a Rust harness that runs F-Droid apps under a **hard fork** of
Android Translation Layer (ATL), classifies failures into signatures, ranks
them, gates on regressions, and generates prompts for an automated fix loop.
Start with `HANDOFF.md` (current state), then `README.md` and `docs/roadmap.md`.

## Hard rules
- **Nothing goes to upstream ATL, and nothing comes back.** Upstream
  (postmarketOS developers) has a no-AI policy; the communities will never
  share code. Never open PRs or issues upstream, never suggest
  upstreaming, never add upstream as a push target, and never fetch/merge/
  rebase upstream code into our branches (D20). Base work on the fork's
  own lineage. See `docs/decisions.md` D1 and D20.
- Don't fake fixes in the ATL fork (catch-alls, `return null`, empty
  bodies). The harness exists to measure real compatibility. Where ATL truly
  can't provide something, return what Android returns in that case and say
  so in a comment.
- Apps run sandboxed (`docs/sandbox.md`); don't widen a sandbox policy to
  make an app work without recording why.
- **A fix nobody can exercise needs an app fetched for it.** If the issue you
  are fixing has zero hits in the current corpus (`apps`/`dex` columns of
  `scripts/atl-gaps` or `atlh missing-apis` say 0), a 0-regression gate
  proves nothing about it. Where possible, fetch an F-Droid app that does
  hit it (`atlh fetch --pkg PKG` on a runner; add the package to a
  `corpora/<name>.txt` list) and A/B it on the old and new build so the fix
  shows as an improvement, not just as no breakage. If no suitable app
  exists, say so in HANDOFF/commit instead of implying the fix was verified.

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

## Several agents at once (Claude, opencode, the overnight loop)
- **One git worktree per agent and task**, never a checkout another agent
  uses: `git -C ~/Develop/android-translation-layer worktree add
  ~/Develop/atl-<task> -b <branch> origin/main`, build it with
  `rig build --worktree ~/Develop/atl-<task>`. `~/Develop/atl-agent` belongs
  to the overnight loop.
- **One agent per runner at a time**: `rig build` installs into that
  runner's shared `/usr/local`, so another agent's build changes what your
  run or gate tests. Say which runner you use in HANDOFF.md ("Runner use")
  and check `rig -r NAME job list` first.
- **Commit only your own files** (`git add <paths>`, not `git add -A`) when
  others have uncommitted work in the same tree.
- Merges to the fork's `main`: only after the gate (0 regressions) and the
  task's interaction script pass; say what was run in the merge message.

## Writing it down
- Commit bodies: root cause, evidence (log lines, what ruled what out), how
  it was verified. Trailer `Assisted-by: opencode (...)` or Claude's.
- Update the task brief in `docs/agent-tasks/` (status, root cause,
  follow-ups) and HANDOFF.md; credit work accurately.

## ATL fork gotchas
- `src/api-impl` is dexed with `dx`: no lambdas or method references
  (invokedynamic), use anonymous classes.
- **A new Java class must be added to the explicit `srcs` list in
  `src/api-impl/meson.build`.** A file that is not listed and not referenced by
  a listed source is silently left out of `api-impl.jar` (javac only pulls it
  in through `-sourcepath`), and the only symptom is the app still dying with
  the `ClassNotFoundException` you just fixed. Verify with
  `python3 -c "import zipfile;print(zipfile.ZipFile('/usr/local/lib64/java/dex/android_translation_layer/api-impl.jar').read('classes.dex').count(b'android/widget/SearchView'))"`
  on the runner (0 = not compiled in).
- New Java `native` methods: commit the JNI header in
  `src/api-impl-jni/generated_headers/` (worktree builds may not regenerate
  it; copy the format of an existing header).
- Never `gtk_snapshot_transform_matrix` for Android matrices (Cairo paints
  them pink): use `snapshot_transform_android_matrix` (fork doc/Shaders.md).
- Debugging: `ATL_BINDER_TRACE=1`, `ATL_DUMP_NODES=DIR`,
  `scripts/runner/jdb-app.sh`, `atlh missing-apis`.
- When an app's own logic is in question, **read its source before its dex**:
  nearly everything on F-Droid is open source and lists its upstream repo in
  the app description (and `SourceCode` in the fdroiddata metadata). Clone it
  and check out the release matching the installed APK (match `versionCode`
  in e.g. `app/build.gradle*`); dex-scanning (androguard) is the fallback,
  not the first step.

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
