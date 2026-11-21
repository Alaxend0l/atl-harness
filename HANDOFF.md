# Handoff: state of atl-harness (2026-11-21, end of day 1)

Read this first, then `AGENTS.md`, `docs/roadmap.md` and `docs/rig.md`.
Continuing without Claude: `docs/opencode-strata.md`.

## What this is
`atlh` runs F-Droid apps under our **hard fork of Android Translation Layer**
(ATL), classifies failures, ranks them, gates on regressions. The fork is
fixed against those results. Nothing ever goes to upstream ATL (AGENTS.md).

## Machines (`runners.toml`, `scripts/rig list`, `scripts/rig doctor --all`)
| Runner | What | Reach | Notes |
|---|---|---|---|
| `bazzite` (default) | x86_64 Bazzite box, Fedora 44 distrobox `atl`; also runs **strata** (local LLM, :8080) | ssh `atl-box` (192.168.10.177) | gates run here (16 cores, `--jobs 8`) |
| `mini` | aarch64 Fedora 44 Lima VM on the M4 Mac mini | ssh `atl-mini-vm` via ProxyJump `atl-mini` (192.168.10.160) | run/screenshot target only, never agents. Lima regenerates the VM's host keys on every VM start: re-pin after verifying via `limactl shell atl -- cat /etc/ssh/ssh_host_ed25519_key.pub` on the mini |
| `oneplus12` | the owner's OnePlus 12 (Fedora 45 from `fedora-atomic-phone`), USB networking | ssh `atl-oneplus12` (172.16.42.1, user `phone`) | everything lives in `/home/phone`: rig rootfs `~/atl-root` via bwrap. Follow the phone project's safety rules. `sudo` needs the owner's password in a real terminal |

ssh to runners is multiplexed (`ControlMaster` in `~/.ssh/config`): fast, and
avoids the OpenSSH per-source penalties that locked the mini's VM out twice.

## Repos
| Repo | Where | State |
|---|---|---|
| atl-harness (this) | github `Alaxend0l/atl-harness`, private | `main` pushed |
| ATL fork | github `Alaxend0l/android-translation-layer`, private; workstation clone `~/Develop/android-translation-layer` | `main` = `f4fe2012`; upstream fetch-only. Runners pull with read-only deploy keys (alias `github-atl-fork`) |
| fedora-atomic-phone | github `Alaxend0l/fedora-atomic-phone`; clone `~/Develop/fedora-atomic-phone` | branches **`plasma-mobile-spectacle`** and **`oneplus12-flatpak-home`** pushed, not merged |

## ATL fork: what's merged on `main`
All gated with the corpus (52 apps on bazzite) unless noted:
1. SoundPool `load(String,int)`, 1-based IDs, load callbacks; `setShowWhenLocked`/`setTurnScreenOn` (Fossify Camera no longer crashes; blank = no camera).
2. Exceptions from app code: lifecycle → uncaught handler + exit 10 like Android; callbacks → logged as `W/ATL: swallowed exception in <fn>`, cleared (was: CheckJNI aborts). +6 apps.
3. TextView honours `android:gravity`/`setGravity` (Calculator labels centered).
4. Drawables: alpha through layer/ripple drawables; cached drawables no longer shared between views (Calculator digit/operator shades).
5. Password masking, input types, `InputType`, `Replacement`/`SingleLine`/`HideReturns`TransformationMethod. **Masking not yet seen on screen** (no corpus app shows a password field at startup; harness can't type yet).

On the phone: ATL `main` at `049eedbe` (everything above) is installed. Note for
fork commits: the build regenerates `src/api-impl-jni/generated_headers/*.h`
with `javac -h`; commit the regenerated header with any new native method.

## Results to know
- Fossify suite (`corpora/fossify.txt`): **13/16 work** on the phone; Phone (telecom), Launcher (app widgets), Camera (no camera) don't. 13 are on the phone's home screen ("… (ATL)"), via `rig desktop-apps`.
- Messages opens but can't send/receive: no default-SMS role, no telephony backend (owner decision pending).
- microG: template + A/B tooling work (`--data-template`), no app reaches Play Services yet. ATL bug: microG native libs path (`<apk file>_` vs `<package>_`).
- Owner's wishlist app **Flow** (YouTube client): starts, then exits on missing `AppWidgetManager.getInstalledProviders()`.

## Next steps (roadmap has the full queue)
1. **Audit fixes**: `docs/audit/2026-11-21-atl-audit.md` (139 confirmed, 41 high; leniently verified, re-check each). Done: password masking. Next suggested: EditText `getText()` returns a copy + Enter sends wrong action; CompoundButton `setOnClickListener` no-op; SharedPreferences listeners; LayerDrawable `isStateful`/`onStateChange` (pressed/disabled looks); gradient shaders + `drawRect` stroke; Bitmap pixel channel swap / `getPixel` exit.
2. App widgets (`AppWidgetManager`, `AppWidgetHost`): unblocks Flow and Fossify Launcher.
3. Visual check of password masking on the phone (an app with a password field).
4. Harness: input scripting, stop-early settle (census is 98% fixed 25 s waits), per-app bubblewrap sandbox (roadmap step 4, owner wants it early).
5. Owner: run the Flatpak move on the phone (see below), install Spectacle, merge the two fedora-atomic-phone branches if happy.

## Waiting on the owner
- Telephony/SMS design for Messages and Phone.
- Merging the two fedora-atomic-phone branches (`plasma-mobile-spectacle`, `oneplus12-flatpak-home`); both are already applied on the phone by hand.

## Done by the owner (2026-11-21)
- Spectacle installed on the phone; `scripts/phone-shot` verified (3/3 captures).
- Flatpak data moved to /home with `move-flatpak-to-home.sh`: root 97% → 52% full (3.7 GB free).
- License chosen: atl-harness is AGPL-3.0-or-later (decisions D15, LICENSING.md).

## Tools added today (all in docs/rig.md)
`rig` (runners, sync, atlh, jobs, bootstrap, rootfs, build, smoke, desktop-apps, doctor), `rig build --worktree` (2–7 s edit→installed, parsed errors), bounded waits (exit 124 timeout / 125 stalled / 126 unreachable; every subprocess has a time limit), `atlh missing-apis`, `scripts/phone-shot`, `corpora/*.txt`, microG data templates.

## Lessons (cost time today)
- `pkill -f PATTERN` from a shell whose command line contains PATTERN kills that shell itself (exit 144/255). Use `pgrep` + exact PIDs.
- Piping a run into `head` kills it (SIGPIPE). Run long things as `rig ... --detach` jobs.
- `rsync -a` copies mtimes: with clock skew cargo skipped rebuilds. `rig sync` doesn't copy mtimes.
- Branch switches while a worktree build/sync runs produce mixed trees: one branch per git worktree when running things in parallel.
- An app still dumping threads after an ART abort looks "alive"; the harness now classifies that as CRASH. ATL's light window frame made blank dark screens look non-blank; the harness crops it.
- The `!` prompt in Claude Code has no TTY: `sudo` there can't ask for a password.
- Background waits outside `rig job wait` can hang silently: always use bounded waits.
