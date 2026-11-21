# Working on atl-harness with opencode + strata

How to continue this project with **opencode** driving the **local model on
strata** instead of Claude. Everything an agent needs is already in the repo
(`AGENTS.md`, `docs/`, `scripts/rig`); this page covers the opencode side.
Verified 2026-11-21 against opencode v2.3.0.

## What's set up

| Thing | Where / value |
|---|---|
| opencode | `~/.opencode/bin/opencode` (v2.3.0) on the workstation |
| Config | `~/.config/opencode/opencode.json` (provider `strata`, default model) |
| Endpoint | `http://192.168.10.177:8080/v1` (OpenAI-compatible) on the **bazzite** runner box |
| Model | `strata/qwen3.8-flash-next-iq3_s` (Qwen3.8-Flash-Next, IQ3_S quant): 262K context, ~155K usable input, 32K output, reasoning low/medium/high, text+image input |
| API key | `~/.config/opencode/secrets/strata.key` (referenced as `{file:…}`; never paste it anywhere) |
| Global rules | `~/.config/opencode/AGENTS.md` (English replies, web search budget) |
| Project rules | `AGENTS.md` in this repo, read automatically (`CLAUDE.md` just includes it) |

Secrets in `~/.config/opencode/` (`secrets/`, `service.json`) stay out of
git, docs and prompts.

## Check strata is up
```sh
curl -s -m 5 -H "Authorization: Bearer $(cat ~/.config/opencode/secrets/strata.key)" \
  http://192.168.10.177:8080/v1/models | python3 -m json.tool | head
```
Expect `qwen3.8-flash-next-iq3_s` with status `loaded`. Strata shares the
bazzite box with the harness: a big census (`rig atlh run` with many jobs)
slows inference, and the reverse. Run censuses on `mini`/`oneplus12`, or
lower `jobs`, while an agent session is active.

## Interactive sessions
```sh
cd ~/Develop/atl-harness
opencode                      # TUI; uses the default strata model
```
Start every session the same way, so the model loads the same context:

> Read AGENTS.md, README.md, docs/roadmap.md and docs/rig.md. Then: <one task>.
> Work on a branch. Before committing run cargo fmt, cargo clippy
> --all-targets, cargo test and python3 -m unittest discover -s scripts/tests.
> Use scripts/rig for anything on runners. Stop and ask before anything
> outside the repo or the runners' home directories.

## Scripted runs
```sh
opencode run --title "rank census1" "Run scripts/rig atlh rank census1 and summarise the top 5 signatures."
opencode run -f /tmp/workitem.md --title "fix: <signature>" "Do the work item in the attached file."
opencode run -c "continue"            # resume the last session
```
`--auto` approves every tool permission that isn't explicitly denied. Only
use it in a throwaway worktree (`--worktree`) or for read-only tasks; never
for anything that touches `~/.ssh`, runners' system files or git pushes.

Work items come from the harness:
```sh
scripts/rig atlh workitem census1 -n 1          # writes work/workitem-census1-1.md on the runner
scripts/rig sh 'cat work/workitem-census1-1.md' > /tmp/workitem.md
```
`[agent].command` in `config.toml` is meant for the future agent loop
(`docs/design.md`, stage 4); it isn't wired up yet.

## Habits that keep a smaller model on track
- **One task per session**, small diffs. A fresh session beats a long one;
  compaction is on (`compaction.buffer` 20000) but summaries lose detail.
- **Make it prove things**: run the test, run `rig doctor`, show the output.
  Don't accept "should work".
- **Give it the commands** instead of letting it invent ssh/rsync lines:
  `rig sh`, `rig atlh`, `rig job`, `rig smoke`, `rig pull`.
- **Review every diff** before it's committed (`git diff`), especially
  anything in the ATL fork: no catch-alls, no `return null`, no empty bodies
  (AGENTS.md hard rules). Nothing ever goes to upstream ATL.
- Commit trailer for its work, matching the repo's convention:
  `Assisted-by: opencode (strata qwen3.8-flash-next-iq3_s)`.
- Hard or cross-cutting work (new subsystems, sandboxing, kernel/phone
  interactions) is where a bigger model helps most; queue it in
  `docs/roadmap.md` rather than forcing it.

## Where the project stands
Status and the next steps live in `docs/roadmap.md`; reasons for past
choices in `docs/decisions.md`; runner operations in `docs/rig.md`;
machine setup in `docs/setup.md`. Keep those current at the end of each
session (AGENTS.md asks for it), since they are what the next session reads.
