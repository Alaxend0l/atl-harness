# Usage

All commands take `--config <path>` (default `config.toml` in the current
directory). From the workstation prefix them with `scripts/rig atlh` (see [rig.md](rig.md));
on the box use `cargo run --release -- <command>` or the built binary
`target/release/atlh`.

## Commands

### `fetch` — build the corpus
```
atlh fetch [--limit 50] [--seed 1] [--refresh-index] [--pkg PKG...] [--pkg-file FILE]
```
Downloads F-Droid's `index-v2.json` (cached in `work/`), picks the latest
version of each app, keeps apps with no native code or with native code for
an ABI in `allowed_abis`, shuffles deterministically with `--seed`, and
downloads up to `--limit` APKs that are not already in the corpus (sha256
verified). Run it again to grow the corpus; existing apps are kept.

With `--pkg`/`--pkg-file` it fetches exactly those packages instead (no
limit, no shuffle) and lists any it can't get (not in the index, or no APK
for this runner's ABIs). Package files hold one package per line, `#`
comments allowed; hand-picked lists live in `corpora/`.

### `run` — run apps under ATL
```
atlh run [--run-id ID] [--jobs 2] [--limit 0] [--pkg PKG...] [--pkg-file FILE] [--force]
         [--data-template DIR | --no-data-template]
```
Runs every corpus app (or just `--pkg …`) and records one row per app under
`--run-id`. Use the ATL commit as the id so runs map to code:
`--run-id $(git -C ~/src/atl rev-parse --short HEAD)`. Default id is
`run-<unix time>`.

Runs are resumable: apps already recorded in that run are skipped unless
`--force`. Each job uses its own Xvfb display, so `--jobs` is bounded by CPU
and RAM, not displays. Start with 2 and raise it while watching load.

**Data template (microG).** Each app starts with an empty ATL data dir.
`--data-template DIR` (or `[atl].data_template`) seeds it from `DIR` first;
read-only files are hard-linked when on the same filesystem, the rest is
copied, so runs never change the template. `rig bootstrap --only microg`
builds `~/src/atl-data/microg`: microG GmsCore from microG's F-Droid repo
(sha256-checked) installed with ATL's `--install-internal`. With it, ATL
loads real microG in-process instead of its built-in GMS stub (`ATLGStub`).
Compare both on the same apps:
```sh
atlh run --run-id mg-gstub  --pkg-file corpora/microg.txt --no-data-template
atlh run --run-id mg-microg --pkg-file corpora/microg.txt --data-template ~/src/atl-data/microg
atlh compare mg-gstub mg-microg
```
First result (2026-11-21): no difference on either runner; none of the 14
apps in `corpora/microg.txt` reaches Play Services code before crashing or
settling. Known ATL bug affecting microG: `main.c` names data dirs after the
APK file (`com.google.android.gms.apk_/lib`) but `ATLLoadedAppManager`
looks for native libs in `<package>_/lib`, so in-process microG loads
without its native libraries.

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
R=scripts/rig
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
`tmux`), since a foreground `rig atlh` dies with the SSH session. Better:
`rig atlh --detach NAME run ...` then `rig job wait NAME`.

## The `work/` directory
```
work/
  atlh.sqlite          all results
  index-v2.json        cached F-Droid index
  apks/                corpus APKs
  runs/<run-id>/       <pkg>.log (ATL stdout+stderr), <pkg>.png (screenshot)
  workitem-*.md        generated prompts
```
`work/` is git-ignored. `rig pull` copies it back (to `work/remote/RUNNER/`) without the APKs or
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
| `atl.data_template` | Optional dir copied into each app's ATL data dir (e.g. `~/src/atl-data/microg`); `run --data-template`/`--no-data-template` override |
| `display.xvfb_args` | Extra Xvfb args (screen size/depth) |
| `display.crop_top_px` | Rows skipped at the top of the screenshot when scoring blankness (ATL title bar); default 0 |
| `classify.stub_patterns` | Regexes ([Rust `regex` syntax](https://docs.rs/regex)) for stub log lines |
| `classify.blank_stddev_threshold` | Screenshot std-dev below which a screen counts as blank |
| `agent.command` | Agent CLI; `{prompt}`, `{prompt_file}` substituted (not used yet) |
| `agent.atl_repo`, `agent.aosp_frameworks` | Source paths given to the agent; `~` expanded |
