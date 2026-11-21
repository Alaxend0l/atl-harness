# Roadmap and status

_Last updated 2026-11-21._

## Status

| Stage | State |
|---|---|
| 1. Census: fetch, run, classify, rank | **Written, unit-tested, not yet run against real ATL** |
| 2. Reference oracle (Waydroid) | Designed only |
| 3. Regression gate | `compare` implemented; no flake handling yet |
| 4. Agent loop | Prompt generation (`workitem`) implemented; orchestrator not started |

Tested so far: classifier parsing and rank/compare SQL (`cargo test`).
End to end on the box (x86_64 Bazzite, `atl` distrobox, upstream ATL
`22a96b28`): `fetch` against the live index, `run` under Xvfb, `rank`. A
3-app smoke run gave 3 × BLANK with 6 signatures, no blind spots.

## Next steps, in order

1. **Box bring-up.** Install packages and Rust, build upstream ATL, `sync`
   and build atlh. ([setup.md](setup.md)) *Done.*
2. **First census.** `fetch --limit 20`, `run`, then work through the
   [first-run checklist](setup.md#first-run-checklist): ATL command, Xvfb
   rendering, stub patterns, blank threshold, blind spots. Update config
   and classifier, then scale to 100–500 apps.
3. **Create the ATL fork repository.** Clone upstream, add it as the
   `upstream` remote, push to a private repo of your own, and point
   `[agent].atl_repo` at the box checkout. Record upstream's license.
   *Repo created (2026-11-21): `github.com/Alaxend0l/android-translation-layer`,
   private, `main` = upstream `22a96b28` + fork notice; upstream is
   fetch-only; license GPLv3. Workstation clone: `~/Develop/android-translation-layer`.
   Runners pull it with per-runner read-only deploy keys (alias
   `github-atl-fork`) and build `main`. Done.*
4. **Flake handling.** Make `compare` (or a new `gate` command) rerun
   regressed apps N times automatically and only fail on consistent
   regressions.
5. **Signature history.** Per app, track how the top signature changes
   between runs so "crash moved deeper" counts as progress.
6. **Agent orchestrator** (`atlh agent`): take the top signature not yet
   attempted, create a worktree, write the work item, run `[agent].command`,
   build ATL, rerun affected apps, then the full corpus, then the gate. Push a
   branch to the fork on success. Record attempts in a new table.
7. **Patch lint for agent diffs**: flag new `return null;`, empty bodies,
   and catch-all catches in framework code.
8. **Reference oracle**: run the corpus on Waydroid, mark apps that don't
   work there (or need Play Services) as out of scope, and later compare
   screenshots.
9. **Input scripting**: drive apps past their first screen (taps,
   scrolls), so later code paths get exercised.

## Known limitations
- Only startup is tested: one screenshot after `settle_s`, no input.
- Blank detection is a single std-dev threshold.
- `fetch` loads the whole F-Droid index into memory (tens of MB; fine).
- Long runs: `rig atlh --detach NAME ...` (jobs survive ssh drops).
- New Xvfb per app costs ~1 s; fine for now.

## Known ATL bugs found by the harness (fork work queue)
- **microG native libs not found**: `main.c` data dir `<apk file>_` vs
  `ATLLoadedAppManager` `<package>_/lib` (see usage.md, data template).
- **JNI glue ignores pending exceptions**: ATL native code calls
  `GetMethodID`/`FindClass`/`GetObjectClass` with an exception pending, so a
  missing API becomes a CheckJNI abort (8/20 apps in census1 on aarch64,
  signature `signal: SIGABRT: JNI ... called with pending exception`).

## Agent tasks
Standing work for coding agents, independent of the steps above:
- [Runner registry, harness tooling, documentation](agent-tasks/tooling-and-docs.md)
