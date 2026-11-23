# Launcher and packaging design (stage plan, nothing built)

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

Implements evidence step 4 of
[`docs/research/vm-launcher-distribution.md`](research/vm-launcher-distribution.md)
(§7): the launcher/daemon design. **This is not a release plan.** Andrun is
pre-alpha — a meaningful share of the corpus still crashes, audio and display
paths in VMs are only partly proven, and there is no security review. The
purpose of this doc is to fix the *shape* of the code so that shipping later
as `.exe`/MSI (Windows), `.dmg`/`.app` (macOS), Flatpak or AppImage (Linux)
stays possible, and to record what must be spiked before any of that starts.

## 1. Two distribution families — keep both alive

| Family | What it is | Formats |
|---|---|---|
| **A. Native Linux** | Andrun runs directly on the host (today's runner boxes) | Flatpak, AppImage, distro packages |
| **B. VM wrapper** | Andrun runs inside a versioned guest image the launcher owns | Windows `.exe`/MSI over WSL2, macOS `.app`/`.dmg` over Virtualization.framework, Linux/QEMU later |

The mistake to avoid is designing only for B (it's where the user reach is)
and letting A rot: A is the developer preview, the CI target and the
donation-build path, and its packaging constraints are the cheapest rehearsal
for B's. Everything below is derived from "would this break A? would this
break B?"

## 2. Layering: three contracts, one per boundary

```
front-end   CLI / drop-directory (GUI only much later)
   │   "run this APK, give me a window" — same verbs on every platform
runner      host adapter: native | qemu | wsl2 | vz   (exactly one per host)
   │   VM contract: boot a guest image, expose the guest protocol
guest       one versioned rootfs (bootstrap script → raw for VZ/QEMU, tar for WSL)
            containing: Andrun build + drop-dir daemon + runner agent
```

- **Guest contract** (guest image version ↔ launcher protocol version): today
  it is plain ssh + a small `atlh`-like CLI, already proven under both Lima
  and bare VZ (§8 of the research doc). Keep the protocol version-negotiated
  from day one; swap ssh for vsock/agent only when a GUI needs it. One
  bootstrap recipe, every artifact derived from it — never a per-host guest
  fork. The image stays unbranded and SPDX-clean (research doc §6).
- **Runner contract**: `launch(apk) → window`, `stop`, `status`, `guest
  version`. Host-specific code lives in one adapter leaf each
  (`host/vz`, `host/wsl`, `host/qemu`, `host/native`); no shared code imports
  a host API. The spike launcher `scripts/research/vzsnd.swift` is the seed
  of `host/vz`.
- **Front-end contract**: CLI + drop-directory only, identical command names
  across platforms. No GUI before the packaging gates below.

## 3. Never-do list (each item closes a door someone asked to keep open)

- **Never require root or fixed system paths at run time.** `rig build`
  installs to `/usr/local` — that is dev tooling, not product layout. Shipped
  builds must run from a relocatable tree with state in per-user dirs (XDG on
  Linux, `%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on
  macOS). Guest images live under the app's data dir, never in
  tool-specific dotdirs (`~/.lima` is dev-only).
- **Never bake Lima into anything.** The VZ spike proved the guest needs no
  Lima; Lima stays a workstation convenience.
- **macOS**: keep the real `.app` bundle layout (Contents/MacOS + Resources
  + the entitlements plist checked into the repo) so the DMG → notarization →
  MAS escalation path stays open. Never put the guest disk in the code
  bundle (size + notarization cost): first-run download into the data dir.
- **Windows**: all WSL interaction behind `host/wsl`; install must be
  silent-capable and per-user (`wsl --import` into the user's distro list),
  so exe/MSI/winget remain free choices. Guest tar URL pinned per launcher
  version.
- **Flatpak (family A)**: Andrun's own bwrap app-sandbox nests inside the
  Flatpak sandbox — this needs a spike (user namespaces inside flatpak; if it
  fails, the existing `--no-sandbox` escape plus flatpak `filesystem=host-osc`
  style perms is the fallback, recorded as a policy weakening per AGENTS
  rule). Keep the sandbox code in one leaf (`src/sandbox*`) so it can be
  probed and disabled per packaging. Never depend on a system ATL/Andrun
  install: bundle the runtime.
- **AppImage (family A)**: free if the above holds — the rule to keep in mind
  is "find everything relative to `$APPDIR`", no daemon at install time.
- **Signing/updates**: stubs only, off by default, in their own module, so
  signing (Apple/SmartScreen) and an updater can be added without reshaping
  the tree. Licensing: fork is GPLv3, harness AGPL-3.0 — any bundle must carry
  the source offer and reproducible checkout scripts.
- **Never let the harness (atlh) become the product's only front-end.** The
  harness keeps its gate/census role; product verbs go through the
  front-end contract so harness churn doesn't break distribution.

## 4. Known gaps that need spikes before packaging (honest list)

1. **macOS display path — biggest unproven item.** The VZ spike was headless
   (ssh only). A product needs guest pixels in a host window: VZ virtio-GPU
   (macOS 15+, API support uncertain for Linux guests) vs `ramfb` + a
   framebuffer viewer. Spike on mini before any `.app` work.
2. **WSL2 spike (research step 2, owner todo)**: `wsl --import` of the guest
   tar, WSLg per-app windows, Pulse glue for AudioTrack.
3. **Audible audio** on a Mac with speakers (the chain is proven to the VZ
   device, not to ears).
4. **Flatpak nesting** of bwrap (above).
5. ARM-Windows: x86-NDK apps under in-guest emulation (dex-only corpus is
   fine today; don't promise ARM yet).

## 5. Stage gates (no dates)

- **Stage 0 (now, internal)** — harness-driven runs on real Linux + mini.
  Nothing new to build; keep the spike scripts honest.
- **Stage 1 (dev preview)** — after gaps 1–2: introduce the tree shape
  (`front/`, `host/*`, `guest/` recipe) in the Andrun repo; `andrun run
  app.apk` works CLI-first on Linux (native), macOS (`host/vz`, headless →
  display window), Windows (`host/wsl`). Family A gets a workable
  AppImage + Flatpak build from the same tree.
- **Stage 2 (packaging rehearsal, still not a release)** — produce `.dmg`,
  `.exe`/MSI, flatpak and AppImage from one tree; install each on a clean
  machine; one witness app reaches a visible window everywhere. That is the
  gate for *calling* anything a public preview.
- **Release criteria (owner sets, kept in roadmap)** — a named corpus
  percentage alive + interacting, audio heard on every host family, sandbox
  review current, telemetry/update posture decided. Andrun's readiness, not
  the launcher's, is the binding constraint.

## 6. Explicit non-goals of this stage

No GUI toolkit, no auto-update, no store submissions, no installer UX, no
telemetry, no code-signing identities. Where those eventually attach is
marked above; none of them may pull shape back into the shared layers.
