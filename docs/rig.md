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
| Rebuild ATL after changing it | `rig rebuild-atl` · `rig rebuild-atl --ref my-branch` · `--pull` |
| Start/stop a Lima VM runner | `rig up` · `rig down` |

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
2. Plain Fedora host: nothing else. Immutable host (Bazzite, Silverblue):
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

## runners.toml
```toml
default = "bazzite"          # optional

[runners.NAME]
ssh = "alias"                # required: ssh destination
arch = "x86_64"              # required: uname -m (doctor checks it)
config = "config.toml"       # harness config on this runner
dir = "atl-harness"          # checkout, relative to ~ (default)
distrobox = "atl"            # optional: run inside this distrobox
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
