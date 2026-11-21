# ATL compatibility harness — design

Goal: measure, rank and eventually auto-fix what stops real Android apps from
running on Android Translation Layer (ATL), with a regression gate so the
system only ever moves forward.

The harness runs on the **runner box** (a Linux machine on the LAN where ATL
builds and runs). Development happens on a separate **workstation** that
holds the git clone and drives the box over SSH. The coding agent (opencode,
Claude Code, …) is a pluggable command that also runs on the box.
See [setup.md](setup.md) for the machine roles.

```
 workstation ──ssh/rsync──► runner box
                             ├── ATL fork checkout (built from source, git worktrees per fix)
                             ├── atlh (this repo, Rust; built with cargo on the box)
                             │     fetch ─► corpus of F-Droid APKs
                             │     run   ─► Xvfb + ATL per app ─► log + screenshot
                             │     rank  ─► failure signatures by impact
                             │     compare ─► regression gate (exit 1 on regression)
                             │     workitem ─► prompt file for the agent
                             └── agent CLI (opencode run / claude -p) edits ATL worktree
```

## Stages

### 1. Census (implemented)
- `fetch`: sample N apps from the F-Droid index (`index-v2.json`), keep only
  APKs whose native code (if any) matches the box's ABI, verify sha256.
- `run`: per app, a fresh Xvfb display and a throwaway `$HOME`/XDG dirs so ATL
  state never leaks between apps. Launch ATL, wait `settle_s`, screenshot,
  kill the whole process group.
- Outcomes: `ALIVE` (running, non-blank screen), `BLANK` (running, uniform
  screen), `EXITED` (exit 0 early), `CRASH` (non-zero / signal),
  `NO_SCREENSHOT` (harness couldn't capture).
- classification (`src/classify.rs`): turn logs into **signatures**:
  - missing-API errors (`NoSuchMethodError`, `ClassNotFoundException`,
    `UnsatisfiedLinkError`, …) → `type: normalized message` — the message
    names the missing method, which is the most actionable signal.
  - other exceptions → `type @ first android.*/com.android.* frame` of the
    root cause.
  - stub log lines (configurable regexes) → `stub: line`.
  - native signals → `signal: SIGSEGV`.
- `rank`: order signatures by **sole blocker** (broken apps where this is the
  only failure seen — fixing it likely unblocks the app), then broken apps,
  then total apps. Also reports broken apps with *no* signature — that number
  tells you how blind the classifier is.

### 2. Reference oracle (next)
Run the same corpus on real Android (Waydroid on the same box, or the
emulator) with the same settle time and screenshot. Only apps that are
`ALIVE` on the reference count toward the score; apps that need network
logins or Google Play Services get marked out of scope instead of polluting
the ranking. Later: perceptual-hash / SSIM similarity between ATL and
reference screenshots to catch "renders, but wrong".

### 3. Regression gate (implemented as `compare`)
`atlh compare BASE NEW` scores outcomes (`ALIVE`=3, `BLANK`/`EXITED`/
`NO_SCREENSHOT`=1, `CRASH`=0) and exits 1 if any app got worse. Before
trusting a regression, rerun just those apps 2–3× (`run --force --pkg …`) — GUI
startup is flaky. Later add a CTS subset as a second gate.

### 4. Agent loop (sketched)
```
loop:
  rank latest baseline → pick top signature not already attempted
  atlh workitem SIG → prompt.md (signature, sample traces, affected apps,
                        pointers to ATL + AOSP sources, rules)
  git worktree add atl-wt/<slug> → agent command runs in it with prompt.md
  build ATL worktree → atlh run --run-id <slug> --pkg <affected apps>
  if signature gone/moved deeper in ≥1 app → full-corpus run → compare
  gate passes → push branch to the fork, PR for human review; else discard + record
```
Agent command is configured in `config.toml` `[agent]`, e.g.
`opencode run "{prompt}"` or `claude -p "{prompt}"`. The orchestrator is a
plain script, so the agent is swappable and multiple can be compared on the
same work items.

## Hard fork
Upstream ATL (postmarketOS developers) has a no-AI-contributions policy, so
this is a hard fork and nothing produced by the loop goes upstream:
- **Never** open upstream PRs/issues from the loop or for fork-only bugs;
  report upstream only bugs you reproduce and fix by hand, if at all.
- Use a distinct project name and say clearly in the README that it is an
  unofficial, AI-assisted fork, so users don't take fork bugs upstream.
- Keep upstream's license files and copyright notices; the fork inherits
  ATL's license. Code ported from AOSP keeps its Apache-2.0 headers.
- Track upstream as a git remote and merge it on a schedule (e.g. weekly).
  After each merge run a full census + `compare` against the pre-merge
  baseline; upstream changes go through the same gate as agent changes.
- Expect merge conflicts to grow as the fork diverges. Keep agent changes
  small and in focused commits (one signature per branch) to keep merges
  tractable, and periodically decide whether upstream sync is still worth it.
- Agent branches are merged into the fork's main by you after review.

## Reward hacking — the main risk
The easiest way to "fix" a crash is to stub it out. Defenses:
- Outcome scoring rewards `ALIVE` (non-blank), not "no exception".
- Reference oracle + screenshot similarity (stage 2) catches blank or wrong UI.
- Diff lint on agent patches: flag new `return null;`, empty method bodies,
  and catch-all `catch (Throwable|Exception)` in framework code for mandatory
  human review.
- Prompt rule: implement from AOSP behavior; if not feasible, leave a
  logged stub and say so rather than silently faking it.
- You merge. The loop produces PRs against the fork, never pushes to main.

## Progress metric
"Fixing one crash reveals the next" is progress even if the outcome is still
`CRASH`. Track per app the *signature history*: a fix counts if the target
signature disappears and is replaced by a different (later) one, with no
regression elsewhere.

## Cost estimate
~settle_s + ~5 s overhead per app. 500 apps × 35 s / 4 jobs ≈ 75 min per full
census. Agent iterations should run only the affected apps first and the full
corpus only when that passes.

## Open questions
Things to confirm on the real box are tracked in
[setup.md → First-run checklist](setup.md#first-run-checklist); stage status
and next tasks are in [roadmap.md](roadmap.md).
