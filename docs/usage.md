# Usage

All commands take `--config <path>` (default `config.toml` in the current
directory). From the workstation prefix them with `scripts/remote.sh atlh`;
on the box use `cargo run --release -- <command>` or the built binary
`target/release/atlh`.

## Commands

### `fetch` — build the corpus
```
atlh fetch [--limit 50] [--seed 1] [--refresh-index]
```
Downloads F-Droid's `index-v2.json` (cached in `work/`), picks the latest
version of each app, keeps apps with no native code or with native code for
an ABI in `allowed_abis`, shuffles deterministically with `--seed`, and
downloads up to `--limit` APKs that are not already in the corpus (sha256
verified). Run it again to grow the corpus; existing apps are kept.

### `run` — run apps under ATL
```
atlh run [--run-id ID] [--jobs 2] [--limit 0] [--pkg PKG...] [--force]
```
Runs every corpus app (or just `--pkg …`) and records one row per app under
`--run-id`. Use the ATL commit as the id so runs map to code:
`--run-id $(git -C ~/src/atl rev-parse --short HEAD)`. Default id is
`run-<unix time>`.

Runs are resumable: apps already recorded in that run are skipped unless
`--force`. Each job uses its own Xvfb display, so `--jobs` is bounded by CPU
and RAM, not displays. Start with 2 and raise it while watching load.

Per app: fresh temp `$HOME`, fresh Xvfb, launch ATL, wait `settle_s`; if it's
still running, take a screenshot; then kill the whole process group.

| Outcome | Meaning |
|---|---|
| `ALIVE` | Still running at settle time, screen not blank |
| `BLANK` | Still running, screen nearly uniform |
| `EXITED` | Exited with code 0 before settle time (suspicious) |
| `CRASH` | Non-zero exit or killed by a signal |
| `NO_SCREENSHOT` | Running, but the screenshot failed |

### `rank` — what to fix first
```
atlh rank RUN_ID [--top 30]
```
Prints outcome counts, the number of broken apps with no recognised
failure (classifier blind spots), and failure signatures ordered by:
1. **sole**: broken apps where this is the only signature (fixing it likely
   unblocks the app),
2. **brk**: broken apps showing it,
3. **apps**: all apps showing it.

### `compare` — regression gate
```
atlh compare BASE_RUN NEW_RUN
```
Compares outcomes for apps present in both runs using the score
`ALIVE=3 > BLANK/EXITED/NO_SCREENSHOT=1 > CRASH=0`. Prints improvements and
regressions; **exits 1 if anything regressed**, so it can gate scripts.
Startup is flaky: rerun regressed apps (`run --run-id NEW --force --pkg …`)
before believing a regression.

### `workitem` — agent prompt
```
atlh workitem RUN_ID [-n 1] [--out FILE]
```
Writes a Markdown prompt for the n-th ranked signature: affected apps, a
sample stack trace, pointers to the ATL and AOSP sources, and the rules
against faking fixes. Default output: `work/workitem-<run>-<n>.md`.

## Typical loop (manual, until the agent loop exists)
```sh
R=scripts/remote.sh
$R sync
$R atlh fetch --limit 100
$R atlh run --run-id base --jobs 4
$R atlh rank base
$R atlh workitem base -n 1        # read it, or hand it to an agent
# ... fix in the ATL fork, rebuild ATL on the box ...
$R atlh run --run-id fix1 --pkg <affected apps>
$R atlh run --run-id fix1          # rest of the corpus
$R atlh compare base fix1
$R pull                            # logs/screenshots/db to ./work locally
```
For long runs, start them inside `tmux` on the box (`$R shell`, then
`tmux`), since `remote.sh atlh` dies with the SSH session.

## The `work/` directory
```
work/
  atlh.sqlite          all results
  index-v2.json        cached F-Droid index
  apks/                corpus APKs
  runs/<run-id>/       <pkg>.log (ATL stdout+stderr), <pkg>.png (screenshot)
  workitem-*.md        generated prompts
```
`work/` is git-ignored. `remote.sh pull` copies it back without the APKs or
index.

## Querying results directly
```sh
sqlite3 work/atlh.sqlite
```
```sql
-- outcomes of one app across runs
SELECT run_id, outcome, screen_stddev FROM runs WHERE pkg = 'org.example' ORDER BY id;

-- apps hit by a signature
SELECT r.pkg, r.outcome, r.log_path FROM failures f JOIN runs r ON r.id = f.run_row
WHERE r.run_id = 'base' AND f.signature LIKE '%setElevation%';

-- broken apps with no signature (classifier blind spots)
SELECT pkg, outcome, log_path FROM runs r WHERE run_id = 'base' AND outcome != 'ALIVE'
AND NOT EXISTS (SELECT 1 FROM failures f WHERE f.run_row = r.id);
```

## Configuration reference (`config.toml`)
Unknown keys are rejected, so typos fail loudly.

| Key | Meaning |
|---|---|
| `paths.work_dir` | Data directory, relative to the config file |
| `corpus.index_url`, `corpus.repo_url` | F-Droid repo (any F-Droid-format repo works) |
| `corpus.allowed_abis` | Native ABIs the box can run |
| `corpus.include_native` | `false` = only apps without native code |
| `atl.command` | ATL invocation; `{apk}`, `{pkg}` substituted |
| `atl.settle_s` | Seconds before screenshot + kill |
| `atl.env` | Extra environment variables for ATL |
| `display.xvfb_args` | Extra Xvfb args (screen size/depth) |
| `classify.stub_patterns` | Regexes ([Rust `regex` syntax](https://docs.rs/regex)) for stub log lines |
| `classify.blank_stddev_threshold` | Screenshot std-dev below which a screen counts as blank |
| `agent.command` | Agent CLI; `{prompt}`, `{prompt_file}` substituted (not used yet) |
| `agent.atl_repo`, `agent.aosp_frameworks` | Source paths given to the agent; `~` expanded |
