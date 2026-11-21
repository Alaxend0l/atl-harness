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
