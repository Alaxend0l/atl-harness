# rig: driving runners from the workstation

`scripts/rig` turns runner chores (sync, build, run, smoke test, fetch
results, long jobs) into short commands. It reads `runners.toml` and talks
to each runner over ssh. Python 3.11+ standard library only; nothing to
install. Put `scripts/` on your `PATH` or use `scripts/rig`.

Every command acts on one runner: `-r NAME` (before or after the
command), else `$ATLH_RUNNER`, else `default` in `runners.toml`.

## Everyday commands

| Task | Command |
|---|---|
| What runners exist, which are up | `rig list` |
| Is a runner healthy? | `rig doctor` · `rig doctor --all` |
| Run the harness (syncs first) | `rig atlh fetch --limit 20` · `rig atlh run --run-id base` |
| Long run that survives ssh drops | `rig atlh --detach base run --run-id base` |
| Watch / wait for it | `rig job log base -f` · `rig job wait base` · `rig job status` |
| Rank, compare, work items | `rig atlh rank base` · `rig atlh compare base fix1` |
| Copy results back | `rig pull` (all, no APKs) · `rig pull --run base` |
| Try one APK by hand | `rig smoke path/to/app.apk` · `rig smoke src/atl_test_apks/x.apk -- -l a/b/Main` |
| Shell in the runner's environment | `rig sh` · `rig sh 'ls work/runs'` · `rig sh --home` |
| Try local ATL edits (no commit) | `rig build --worktree ~/Develop/android-translation-layer` (~2–7 s incremental) |
| Build ATL from the runner's checkout | `rig build` · `rig build --ref origin/my-branch` · `--pull` (`rebuild-atl` = same) |
| Just check it compiles | `rig build --no-install` · on every runner: `rig build --all` |
| Other components | `rig build art` · `bionic` · `opensles` · `harness` (`--clean` to start over) |
| Start/stop a Lima VM runner | `rig up` · `rig down` |
| Put apps on a runner's home screen (real session, not Xvfb) | `rig -r oneplus12 desktop-apps install --pkg org.fossify.notes …` · `remove` |
| (Re)install a bwrap rootfs runner | `rig -r oneplus12 rootfs --from mini` |

Details:
- `rig atlh` passes everything after its own options to `atlh` and adds
  `--config` from `runners.toml`, plus `--jobs` for `run` unless given
  (`--no-default-jobs` to skip). To see atlh's own help: `rig atlh -- --help`.
- `rig sh` runs inside the distrobox (if any) with cargo and `/usr/local`
  pkg-config set up, in the harness checkout (`--home` for `~`). Quote the
  command to use pipes or `&&`.
- `rig smoke` uploads a local APK (or takes a path on the runner relative
  to `~`), runs it with the same environment as the harness runner, and
  copies `run.log` and `shot.png` to `work/remote/RUNNER/smoke/NAME/`. It
  prints whether ATL survived `--settle` seconds, whether the activity
  reached `onResume`, and the blankness score (title bar cropped).
- `rig pull` writes to `work/remote/RUNNER/`, so runners never overwrite
  each other's databases.

## Builds
`rig build [TARGET]` runs `scripts/runner/build.sh` on the runner: meson
setup only when needed, compile, install (unless `--no-install`), with the
full log in `~/src/logs/build-TARGET-<time>.log` (`-latest.log` links to
the newest). It ends with `BUILD OK atl in 5s (109 warnings)` or `BUILD
FAILED` plus a deduplicated error list from `scripts/runner/builderrors.py`
(gcc/clang, javac, linker, meson, pkg-config/dnf, cargo, dex, ninja/make),
each with file:line and the offending source lines; `-latest.json` has the
same as JSON for agents. Exit status is the build's.

`--worktree PATH` rsyncs a local ATL tree (uncommitted edits included,
`.git` and `builddir` excluded) to `~/src/atl-dev` on the runner and builds
there incrementally: the edit-compile-test loop without commit/push/pull.
After an install, `~/src/logs/atl-installed.txt` records what is installed
(`<sha>[-dirty]` or `local:<git describe>`); `rig doctor` shows it. Use
`rig smoke`/`rig atlh run` right after to test the build.

## Apps on a runner's own screen
`rig desktop-apps install --pkg PKG…` (APKs from the runner's corpus) adds a
"Name (ATL)" launcher per app to `~/.local/share/applications` with the
icon from the APK or, for release builds with shrunk resources, F-Droid's
published icon (sha256-checked). Launchers run `~/.local/bin/atl-app`,
which starts ATL in the rig rootfs (if any) inside the desktop session:
Wayland, session D-Bus, audio sockets and GPU passed through. Each launch
appends ATL's output to `~/.local/state/atl/PKG.log` for bug reports. App
data persists in ATL's data dir. Unlike harness runs this is not isolated
from the session; use it for trusted apps.

## Jobs
`rig job start NAME -- COMMAND` runs COMMAND detached on the runner (in
the checkout), logging to `~/atlh-jobs/NAME.log` and recording the exit
code. Jobs survive ssh disconnects and distrobox exits. `rig job wait`
polls quietly, rides out network blips, and exits with the job's exit
code. `rig job stop NAME` kills the job's process group. `bootstrap`,
`atlh --detach` and `rebuild-atl --detach` are jobs.

## Adding a runner
1. Make it reachable: an alias in `~/.ssh/config` and
   `ssh-copy-id ALIAS`. Put IPs only in `~/.ssh/config`. Ideally give the
   machine a DHCP reservation.
2. Plain Fedora host: nothing else. Can't install packages at all (full or
   off-limits root partition, e.g. the OnePlus 12): give it a **bwrap
   rootfs**, see below. Immutable host (Bazzite, Silverblue):
   `distrobox create --name atl --image registry.fedoraproject.org/fedora-toolbox:44`.
   Mac: a Lima VM, see [setup.md](setup.md#second-runner-mac-mini-aarch64-lima-vm).
3. Register it; this probes arch and CPU count and picks the config:
   ```sh
   rig add NAME --ssh ALIAS [--distrobox atl] [--lima-host MAC --lima-instance atl]
   ```
4. Give it read access to the private ATL fork (one deploy key per runner,
   read-only, scoped to the fork; decisions D13):
   ```sh
   ssh ALIAS 'ssh-keygen -t ed25519 -N "" -C atl-fork-deploy@NAME -f ~/.ssh/atl-fork_ed25519'
   ssh ALIAS 'cat ~/.ssh/atl-fork_ed25519.pub' | gh repo deploy-key add - \
     -R Alaxend0l/android-translation-layer -t "runner NAME (read-only)"
   ssh ALIAS 'printf "\nHost github-atl-fork\n    HostName github.com\n    User git\n    IdentityFile ~/.ssh/atl-fork_ed25519\n    IdentitiesOnly yes\n" >> ~/.ssh/config'
   ssh ALIAS 'ssh -o StrictHostKeyChecking=accept-new -T github-atl-fork'   # "Hi Alaxend0l/android-translation-layer!"
   ```
   Agents that push branches need `--allow-write` on their runner's key;
   add that only when the agent loop starts.
5. `rig -r NAME doctor` to see what's missing.
6. `rig -r NAME bootstrap --wait`: packages, Rust, wolfSSL,
   bionic_translation, vixl (aarch64), art_standalone, libOpenSLES, ATL,
   atlh, microG data template. Takes 30–60 min, mostly ART. Each step logs to
   `~/src/logs/bootstrap-STEP.log` and is skipped once done; after a
   failure fix the cause and rerun the same command. `--from STEP`,
   `--only a,b`, `--force` to redo.
7. `rig -r NAME doctor` again, then `rig -r NAME smoke
   src/atl_test_apks/org.happysanta.gd_29.apk`.

## bwrap rootfs runners
For machines where nothing may be installed outside `$HOME`. `rig rootfs
--from SRC` builds a Fedora tree with the whole ATL stack on a bootstrapped
runner of the same arch (`scripts/runner/make-rootfs.sh`: `dnf
--installroot` with `scripts/runner/packages.txt`, plus SRC's `/usr/local`),
streams it to `~/<bwrap_root>` on the target, and swaps it in. Every `rig`
command then runs inside it via unprivileged `bwrap`: the tree is `/`, with
`$HOME`, `/tmp`, `/dev/dri`, DNS and the host's passwd/group passed through,
a private `/run/user/UID`, and a **private D-Bus session** (libportal
aborts without one; the host desktop's bus is never exposed). Detached jobs
get their own bus. A `sudo` shim makes bootstrap steps work, since the user
owns the tree. Then mark the steps the rootfs covers as done and bootstrap
the per-user rest:
```sh
rig -r NAME sh --home 'mkdir -p ~/src/.bootstrap; for s in packages wolfssl bionic vixl art opensles atl; do touch ~/src/.bootstrap/$s.done; done'
rig -r NAME bootstrap --only rust,sources,harness,microg --wait
```
Rebuild the rootfs after ATL changes that touch native code or deps:
`rig -r NAME rootfs --from mini` (Java-only changes: `rig rebuild-atl`).

## runners.toml
```toml
default = "bazzite"          # optional

[runners.NAME]
ssh = "alias"                # required: ssh destination
arch = "x86_64"              # required: uname -m (doctor checks it)
config = "config.toml"       # harness config on this runner
dir = "atl-harness"          # checkout, relative to ~ (default)
distrobox = "atl"            # optional: run inside this distrobox
bwrap_root = "atl-root"      # optional (not with distrobox): rootfs under ~, see above
jobs = 4                     # default --jobs for `atlh run`
make_jobs = 6                # optional: compile parallelism for bootstrap
atl_src = "src/android_translation_layer"   # ATL checkout, relative to ~
atl_git = "git@github-atl-fork:Alaxend0l/android-translation-layer.git"  # default: the fork
lima_host = "atl-mini"       # optional, with lima_instance: for up/down/doctor
lima_instance = "atl"
notes = "free text"
```
Unknown keys are errors, like `config.toml`.

## Doctor checks
ssh · distrobox exists · Lima VM running · arch matches · page size is 4K
(see decisions D11) · Fedora · cargo, Xvfb, ImageMagick, ATL on PATH ·
art-standalone installed · ATL checkout, its `origin` is the fork, and
`upstream` can't be pushed to · disk ≥ 20 GB · memory/cpus ·
harness checkout synced with this workstation (hash of `src/`, Cargo
files, `config*.toml`) · atlh built · clock skew ≤ 5 s. Exit code: 0 all
ok, 1 warnings, 2 failures. Every failure line has a fix hint.

## Tests
`python3 -m unittest discover -s scripts/tests`. They need no network or
runner: quoting is checked by running the generated scripts through local
bash, and jobs are started on the local machine.

## Older helper
`scripts/remote.sh` (single runner via `ATLH_REMOTE`/`ATLH_REMOTE_BOX`)
still works but is deprecated in favour of `rig`.
