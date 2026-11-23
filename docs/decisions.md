# Decision log

Newest last. Add an entry when you change direction.

### D1 — Hard fork, nothing goes upstream (2026-11-21)
Upstream ATL (postmarketOS developers) has a hard no-AI-contributions
policy. The ATL fork is therefore a hard fork: no upstream PRs or issues
from this project or for fork-only bugs. Keep upstream license files and
copyright notices; label the fork clearly as unofficial and AI-assisted.
Upstream changes are merged into the fork on a schedule and go through the
same regression gate. See [design.md → Hard fork](design.md#hard-fork).

### D2 — Harness in Rust (2026-11-21)
Considered Python (faster iteration while heuristics are in flux) vs. Rust
(type safety, single binary, sturdier process handling). Chose Rust now
because a later port would only get harder. The ATL fork itself stays in
its own languages (Java framework + C native glue).

### D3 — Two machines: workstation + runner box (2026-11-21)
ATL needs Linux; the harness and agent run on a dedicated Linux box on the
LAN. Development happens on a Linux workstation with the git clone, driving
the box over SSH (`scripts/remote.sh`). The harness has no knowledge of
this split, so a single machine also works.

### D4 — Xvfb + ImageMagick for headless runs (2026-11-21)
Simplest path to a per-app display and screenshot. Assumes ATL (GTK4) works
on X11 with software GL. Untested against ATL. If it doesn't work, switch the
runner to a headless Wayland compositor.

### D5 — F-Droid as the corpus (2026-11-21)
Open-source apps, legally redistributable, with a machine-readable index
that includes ABIs and checksums. Biased toward simpler apps without Google
Play Services, which suits ATL's current maturity.

### D6 — SQLite, single writer (2026-11-21)
One file, queryable by hand with `sqlite3`, easy to copy back with
`remote.sh pull`. Only the main thread writes.

### D7 — Agent is a pluggable CLI command (2026-11-21)
The agent loop calls whatever is in `[agent].command` (opencode, Claude
Code, …) in an ATL worktree. Lets agents be swapped or compared on the
same work items.

### D8 — Gate scores outcomes, not exceptions (2026-11-21)
`ALIVE=3 > BLANK/EXITED/NO_SCREENSHOT=1 > CRASH=0`. Rewarding "no
exception" would invite stub-it-out fixes; rewarding a live, non-blank
screen is harder to game. Stage 2 (reference oracle) tightens this further.

### D9 — Private repo, license TBD (2026-11-21)
`atl-harness` starts private with no license. Decide before making it
public. Note the ATL fork's license is inherited from upstream regardless.

### D10 — Runner box is x86_64 Bazzite; harness runs in a distrobox (2026-11-21)
The runner box (shared with the local strata LLM server) is x86_64 and runs
Bazzite, an immutable Fedora system. Rather than layering ATL's many build
dependencies onto the host with `rpm-ostree` (reboots, messy host), the
toolchain, ATL and the harness live in a Fedora 44 distrobox (`atl`).
`scripts/remote.sh` enters it when `ATLH_REMOTE_BOX` is set. Corpus is
filtered to `allowed_abis = ["x86_64"]`, which drops apps that ship ARM-only
native code.

### D11 — Mac mini M4 is a second runner, via a Lima VM, never an agent host (2026-11-21)
For apps that ship ARM-only native code (~7% of F-Droid), an M4 Mac mini
runs a Fedora 44 aarch64 VM under Lima (Apple Virtualization, `vz`). Asahi
was ruled out: no M4 support, and its 16K-page kernel breaks native libs
built for 4K pages (the VM's kernel uses 4K, like phones). The mini (16 GB) is
too small for useful local models, so it only runs apps and takes
screenshots; agents stay on strata. Results from x86_64 and aarch64 runners
are not comparable and must not be gated against each other. No Apple
Silicon chip runs 32-bit ARM, so `armeabi-v7a`-only apps stay out of reach.

### D12 — Workstation tooling in Python (`scripts/rig`), registry in runners.toml (2026-11-21)
Remote chores (sync, distrobox/VM entry, detached builds, smoke tests,
health checks) had become long hand-typed ssh lines and broke in subtle
ways (quoting, stale builds, jobs dying with ssh). `rig` centralises them.
Python stdlib rather than Rust because it orchestrates ssh/rsync on the
workstation, needs no build, and argument/TOML handling in bash is fragile;
the harness itself stays Rust (D2). Runner-side logic stays in bash
(`scripts/runner/`), sent inline where it must work before a sync.
Per-runner results live in separate DBs (`work/remote/RUNNER/`), so runs
from different architectures can't be compared by accident.

### D13 — The ATL fork: private GitHub repo, `main`, fetch-only upstream (2026-11-21)
`Alaxend0l/android-translation-layer` (private for now). Branch `main` =
upstream `master` at the fork point plus fork commits; upstream changes come
in by `git merge upstream/master`. The `upstream` remote's push URL is set to
an invalid value so nothing can be pushed there by accident (D1). `FORK.md`
and a README banner mark it as unofficial and AI-assisted; GPLv3 and upstream
copyright notices are unchanged.

### D14 — OnePlus 12 runner: rootfs in $HOME via bwrap (2026-11-21)
The phone (Fedora 45 from fedora-atomic-phone) has an 8 GiB, 97%-full root
partition and no podman/distrobox; its system is another project's domain.
Instead of installing anything there, `rig rootfs` builds a Fedora 44
aarch64 tree with the ATL stack on the mini and installs it in
`~/atl-root`; `rig` runs commands in it with unprivileged `bwrap` (private
`/run`, private D-Bus session, `$HOME`/`/tmp`/GPU passed through). This is
also the first step towards per-app sandboxing (roadmap step 4).

### D15 — atl-harness is AGPL-3.0-or-later (2026-11-21)
Replaces D9's "license TBD". Owner's choice, matching fedora-atomic-phone. All
dependencies are permissive (checked with `cargo metadata`), so the combination
is fine. AGPL's network clause only matters if the harness is ever offered as a
service (e.g. a results dashboard); for the CLI it behaves like GPL-3.0. The
ATL fork stays under upstream's GPL-3.0 and is not affected. Details:
[LICENSING.md](../LICENSING.md).

## D16 (2026-11-21): per-app bubblewrap sandbox, policy from manifest permissions
Every app runs in its own bwrap sandbox (`scripts/runner/atl-sandbox`): empty
home plus its own data dir, media folders per Android storage permission,
read-only system, Wayland only, filtered D-Bus (xdg-dbus-proxy), network only
with INTERNET, camera only with CAMERA. The policy is derived from the
manifest at install and kept in an editable JSON file per app. Chosen over
Flatpak-style portals-only storage because Android apps expect real media
folders (MediaStore), and over a hand-written policy per app because the
manifest already states what the app needs. Known gaps (LAN reachable with
INTERNET, mic via PipeWire, no seccomp) are listed in docs/sandbox.md.

## D17 (2026-11-22): local-model fixes stage on `local-fixes`; `main` takes only reviewed merges
Owner instruction after the first full overnight run: fixes produced by the
local model (agent-loop, and hand-finished retries of its queue items) do not
land on the fork's `main` without owner review. The 2026-11-22 merges
(`66ff04af`, `c3b2684b`, `2ea23604`) were unwound: `main` was rewound to
`7cb4cd2b` and all 20 gated fixes now live on `local-fixes`. That branch is
the agent-loop's default target (`--branch local-fixes`) and accumulates
gated local-model fixes stacked on `main`; the morning review is
`origin/main..origin/local-fixes`. Merging into `main` follows AGENTS.md's
merge rule and is the owner's call. `agent/overnight` stays as the raw branch
of run 20261121-185746.

## D18 (2026-11-23): the app sandbox masks /etc/group (Android-fidelity, not a policy widening)
The canary's second flake mode — a JNI abort in `me.zhanghai.android.files`'
own syscall library — turned out to be an environment difference, not a race:
Android has no `/etc/group`, so bionic's `getgrgid` always answers "not
found" and the app's null-handling runs; under bwrap the app saw the *host's*
group database through glibc NSS, took the "lookup succeeded" path no device
takes, and aborted the process parsing the result. `atl-sandbox` now binds
`/dev/null` over `/etc/group`. Evidence: 6 forced canary runs went JNI-mode
2/6 → 0/6 (remaining crashes are the separate, pre-existing NPE mode), and
gate `fc60` vs `fc59` shows `me.zhanghai.android.files` CRASH→ALIVE, 0
regressions across the corpus. Masking `/etc/passwd` the same way was tried
and reverted on the spot: the art_standalone libcore copy the runtime links
throws `AssertionError` from `System.<clinit>` when `getpwuid` fails, so every
app died at startup (6/6 CRASH). AOSP main's `System.java` instead falls back
to `user.name="unknown"` — aligning art_standalone with it is the follow-up
that would let passwd be masked too. This narrows what an app can read (host
account names were leaking), so it is not a sandbox-policy widening in D16's
sense; the rationale lives next to the mount in the script.
