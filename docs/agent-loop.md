<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Unattended agent loop (`scripts/agent-loop`)

Lets the local model (opencode + strata, `qwen3.8-flash-next-iq3_s`) work
through the [audit](audit/2026-11-21-atl-audit.md) overnight on a separate
branch of the ATL fork. Nothing it does reaches the fork's `main`, the phone,
or anything outside one clone; you review its branch in the morning.

## Safety model
| Who | Can |
|---|---|
| **The agent** (`atl-fixer`, defined in the clone's `.opencode/agents/`) | read files; edit files under the clone's `src/` only; run read-only shell commands (`git diff/status/log/show`, `grep`, `find`, `ls`, `head`, `tail`, `wc`). Shell chaining, pipes, redirection, command substitution and `find -exec/-delete` are denied. No network, ssh, builds, commits or pushes. |
| **The loop script** (deterministic) | lint the diff, build on the runner, run the gate, commit, push **only** the agent branch (default `local-fixes`, D17: local-model fixes never land on `main` without owner review), restore the runner's ATL to `origin/main` at the end |

Permission rules were verified empirically on 2026-11-21: writes outside
`src/` and outside the clone, `ssh`, and bypass attempts (`;`, `>`, `$(...)`,
`find -delete`) were all refused; allowed reads/edits/`git status` worked.

**Queue** (`--queue triage`, the default): `docs/agent-tasks/local-queue.json`,
the items a multi-agent triage judged small and unambiguous enough for the
local model, ordered by user impact, each with a precise `fix_hint` that goes
into the prompt ([docs/agent-tasks/triage.md](agent-tasks/triage.md); the
rest is marked for Claude). `--queue audit` takes every audit item instead.
Titles in `docs/audit/fixed.txt` are skipped either way.

Per task the loop: agent edits (time limit, default 30 min) → **self-review** (the agent re-reads its diff against the fix hint and fixes or reverts it; `--no-review` skips) → **lint**
(rejects: changes outside `src/`, catch-all or empty `catch`, removing >150
lines; flags for review: added `return null;`/`return 0;`, empty bodies) →
**build** on the runner (one retry with the parsed compiler errors) →
**gate** on the corpus (`--jobs 4`, strata shares the box) → **commit + push
the agent branch only if** it built, passed lint and nothing regressed;
otherwise the change is discarded. Each accepted change becomes the next
baseline. One failing task never stops the loop.

What it can't protect against: changes that are wrong but don't regress any
corpus app (most audit fixes act on interaction the harness can't drive yet),
or plausible-looking fake fixes the lint misses. **Review every commit.**

## One-time setup (done 2026-11-21)
```sh
git clone git@github.com:Alaxend0l/android-translation-layer.git ~/Develop/atl-agent
cd ~/Develop/atl-agent
git remote add upstream https://gitlab.com/android_translation_layer/android_translation_layer.git
git remote set-url --push upstream DISABLED-hard-fork-never-push-upstream
echo .opencode/ >> .git/info/exclude
# .opencode/agents/atl-fixer.md: see the file in that clone (permissions + rules)
```
The clone is separate from `~/Develop/android-translation-layer`, so your own
work is never touched.

## Running
On the workstation (it needs opencode, strata over the LAN, and rig); the
script keeps the machine from sleeping (`systemd-inhibit`). Use tmux or leave a
terminal open:
```sh
scripts/agent-loop --max-tasks 2 --no-push --hours 2    # supervised trial first
scripts/agent-loop --hours 8                             # overnight
```
When the queue runs low, `scripts/atl-gaps --emit-queue N` (docs/usage.md)
refills it from a static scan of the ATL tree ranked by corpus usage; run it on
a runner so the sqlite/APK joins work.
Options: `--task-minutes`, `--jobs`, `--runner`, `--branch`, `--max-removed`,
`--no-push`. While it runs, bazzite has the agent's ATL build installed; it is
put back to `origin/main` when the loop ends (also after an error or Ctrl-C).

## In the morning
- `work/agent/<time>/summary.md`: one row per task (committed / no change /
  rejected by lint / build failed / regressions), review flags, commits.
- `work/agent/<time>/task-NN/`: the agent's output, the diff, build logs, gate.
- Review: `git -C ~/Develop/atl-agent log -p origin/main..origin/local-fixes`.
  Cherry-pick or merge what's right into the fork's `main` yourself (or ask
  Claude to check them), add their titles to `docs/audit/fixed.txt`.
