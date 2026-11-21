# Architecture

A single Rust binary (`atlh`) with no daemons. All state is in one SQLite
file plus logs and screenshots on disk.

## Code map

| File | Responsibility |
|---|---|
| `src/main.rs` | CLI (clap), `run` orchestration: a worker pool of `--jobs` threads pulls apps from a queue; results go over a channel to the main thread, which owns the DB connection and writes one transaction per app. |
| `src/config.rs` | Typed `config.toml` (serde, `deny_unknown_fields`). Resolves `work_dir` relative to the config file and expands `~`. |
| `src/db.rs` | Schema and connection. Tables: `apps` (corpus), `runs` (one row per app per run id), `failures` (signatures per run row). |
| `src/corpus.rs` | F-Droid `index-v2.json` parsing (typed), ABI filtering, seeded shuffle (ChaCha8, stable across platforms), streaming download with sha256 check. |
| `src/runner.rs` | Runs one app: temp `$HOME`/XDG dirs, a private Xvfb (`-displayfd` picks a free display), ATL in its own process group, wait `settle_s`, screenshot via ImageMagick, kill the group, classify. Defines `Outcome` and its gate score. |
| `src/classify.rs` | Log → `Vec<Failure>`. Parses Java exception chains and stack frames, matches stub regexes, normalises numbers/addresses so signatures group across apps. |
| `src/report.rs` | `rank` (SQL aggregation), `compare` (regression gate), `workitem` (prompt template). |
| `scripts/rig` | Workstation tool (Python stdlib): runner registry (`runners.toml`), sync, remote `atlh`, smoke, detached jobs, doctor, bootstrap. See [rig.md](rig.md). |
| `scripts/runner/*` | Run on runners: `bootstrap.sh` (build stack), `build.sh` + `builderrors.py` (component builds with error summaries), `make-rootfs.sh`, `smoke.sh` (one APK), `doctor.sh` (checks). |
| `scripts/remote.sh` | Deprecated single-runner helper. |

## Data flow
```
F-Droid index ─► fetch ─► apps table + work/apks/
apps ─► run ─► runner::run_one ─► work/runs/<id>/<pkg>.{log,png}
                         └─► classify::parse ─► runs + failures tables
runs/failures ─► rank / compare / workitem
```

## Signatures
A signature is the unit of work: one string that should mean "the same
root problem" across apps.

| Kind | Source | Signature format |
|---|---|---|
| `missing_api` | Root cause is `NoSuchMethodError`, `NoSuchFieldError`, `ClassNotFoundException`, `NoClassDefFoundError`, `UnsatisfiedLinkError` or `AbstractMethodError` | `<type>: <normalised message>` (the message names the missing API) |
| `exception` | Any other exception chain | `<type> @ <first android.* / com.android.* frame>` of the root cause, else its first frame |
| `stub` | Log line matching `stub_patterns` | `stub: <normalised line>` |
| `signal` | Process killed by a signal | `signal: SIGSEGV` etc. |

Rules: `Caused by:` lines join the previous exception's chain and the
**last** cause is the root. Numbers and hex addresses become `N`. Duplicate
signatures within one log are dropped.

## Design constraints worth keeping
- **One DB writer.** Workers never touch SQLite; keep it that way rather
  than adding locking.
- **Isolation per app.** Fresh `$HOME` and display every time so results
  don't depend on run order. Don't share ATL state between apps to save time.
- **Kill the process group, not just the PID.** ATL may spawn helpers.
- **Typed config, strict parsing.** Add new keys to the structs; don't read
  raw TOML values.
- **Outcome scores are the gate's contract.** Changing them changes what
  counts as a regression; note it in [decisions.md](decisions.md).
