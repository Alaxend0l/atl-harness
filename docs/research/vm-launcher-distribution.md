<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# VM-launcher distribution: an exe/DMG that carries Andrun to end users (owner question 2026-11-23 — feasibility, no code written)

Question: could Windows users run an `.exe` and Mac users unpack a `.dmg`,
which silently sets up and self-configures a VM running Andrun, with a
launcher that runs APKs dragged into it — and is that easier than tearing
down the five native-port walls (`graphics-backend.md` §3)?

## TL;DR

**Feasible, and an order of magnitude cheaper than the walls.** Both
platforms already ship the hard parts in the OS:

- **Windows**: WSL2's custom-distribution mechanism is exactly this
  product shape — a `.wsl` file (a tar of our rootfs) that **double-click
  installs** (OOBE), gets Start-Menu shortcuts from `.desktop` entries, and
  gives **per-app windows in the taskbar** via WSLg; audio is a distro
  config item (`/mnt/wslg/PulseServer` + an ALSA `default` → Pulse route).
- **macOS**: Apple's Virtualization.framework (public API, used by Docker
  Desktop, Podman, UTM, Lima) provides a Linux arm64 guest with virtio-GPU
  display, **virtio-sound**, virtiofs shared folders and SPICE clipboard —
  everything a `.app` needs to run our guest image with no bundled QEMU.
- **One guest image serves both**: the rootfs `scripts/runner/bootstrap.sh`
  already builds is the product; Windows wraps it as `.wsl`, macOS boots it
  in a VZ VM. The drag-a-APK launcher is a small tray app + a guest daemon
  on a shared dir/vsock.
- This is **already half-proved in our own fleet**: `mini` is Andrun running
  in a Linux VM on a Mac (apps launch, windows render); the only known gap
  is that VM's sound device config — a VM config item, not Andrun code.

Verdict vs the walls: the walls are a multi-year, AOSP-scale runtime port
with a permanent per-OS maintenance fork; this path is packaging + glue,
keeps **one** runtime (our gates/witnesses stay meaningful on every OS),
and every artifact already exists (bootstrap rootfs, aarch64+x86_64 builds,
Xvfb headless mode). Recommended before committing: three cheap evidence
steps (bottom).

## 1. Windows path (WSL2 + WSLg)

Mechanics verified against Microsoft docs (2026-11-23):

- **Distribution**: a custom distro is "a tar file with a `.wsl` file
  extension"; OOBE supports double-click install, `oobe.defaultName`, and a
  `[shortcut]` section (icon + Start-Menu entry). The exe/MSI is a thin
  bootstrapper: enable check, `wsl --import` (or double-click `.wsl`), then
  launch the tray app.
- **GUI**: WSLg (`guiApplications=true` default) shows each Linux app
  window as a **native Windows window with taskbar icon**; Weston
  enumerates `.desktop` entries → Start-Menu app list "for free". Our
  `atl-app` wrapper gets one .desktop per app = launcher icons.
- **Audio**: WSLg exposes a PulseAudio server at `/mnt/wslg/PulseServer`.
  Andrun's AudioTrack speaks ALSA by *name*, so a distro `/etc/asound.conf`
  routing `default` → the Pulse ALSA plugin (alsa-plugins) is guest config,
  **not Andrun code** (verify our device-name choice honors asound.conf —
  small probe, not a rewrite).
- **GPU**: WSLg's `/dev/dxg` + Mesa D3D12 provides GLES/EGL to guests
  (standard for GPU-accelerated Linux GUI on WSL2).
- **Sandbox**: bwrap needs unprivileged user namespaces — available and
  tunable in the guest (`kernel.unprivileged_userns_clone`), i.e. our
  per-app sandbox works.
- **Prereqs/friction**: Win10 2004+/Win11 with virtualization; first WSL
  enable is one admin action; SmartScreen wants a code-signing cert
  (one-time cost); skip WSL on VBS-locked enterprise machines (fallback:
  bundle QEMU/Hyper-V — heavy, mark unsupported instead).

## 2. macOS path (Apple Virtualization.framework inside the .app)

- **Hypervisor**: VZ is an OS framework — no bundled hypervisor, public
  API; precedent for shipping VMs in normal Mac apps up to and including
  Mac App Store (UTM). Tooling exists to build on (Apple's own
  `container`/`containerization`, `vfkit` as used by Podman/minikube,
  `vfrust` Rust bindings).
- **Guest**: Linux arm64 via direct kernel boot + our rootfs;
  **virtio-GPU** (native `VZVirtualMachineView` display), **virtio-sound**
  (macOS 14+; Fedora's aarch64 kernel already carries the virtio-sound
  module — Docker's missing audio on Mac is *its* guest kernel config
  (`CONFIG_SOUND` off), not a VZ limit), **virtiofs** shared folder for APK
  drops and file intents, SPICE clipboard (macOS 13+). Intel Macs: same
  framework runs x86_64 guests (our x86_64 build exists).
- **Display UX**: v1 = a kiosk Wayland session inside the VM shown in one
  crisp VM window (app windows inside, like UTM); v2 = per-app windows on
  the Mac desktop (X11/Wayland forwarding to a host helper — Docker
  Desktop-style, later nicety, no Andrun changes).
- **Distribution**: Developer ID signing + notarization for the DMG
  ($99/yr); MAS is possible (public APIs) but its update model fights
  auto-updating guest images — DMG first.

## 3. The shared product: one guest, two wrappers

The asset to build is **the guest product**, identical for both OSes:

1. Rootfs from `scripts/runner/bootstrap.sh` (unbranded base — Fedora
  packages fine, Fedora *artwork/trademarks* not).
2. An in-guest **provisioner/daemon**: watches a shared dir (virtiofs /
  `\\wsl$\host\...`) or vsock for dropped APKs → installs → launches
  `atl-app <pkg>` → windows appear through WSLg / the VM display; relaunch
  and app-grid management from the host tray app.
3. Guest glue: `.desktop` entries, asound.conf/Pulse (Windows) or
  virtio-sound (macOS), Weston/kiosk session, bwrap userns sysctl.
4. Host tray app per OS (drag target, app grid, first-run progress,
  update channel for the guest image).

First-run cost: OOBE import/expand ≈ a minute or two (VM disk grows
~4–8 GB, RAM 2–4 GB); subsequent starts are fast (persistent VM; macOS
supports suspend/resume).

## 4. Launcher UX (drag APK → window)

- Windows: drag onto tray-app or straight onto the installed app's Start
  icon via a file association (`andrun.apk` → launcher); the launcher copies
  to the shared dir; the guest daemon installs; WSLg pops a window with a
  taskbar icon. Close window = app exit; tray app manages installed apps,
  storage, updates.
- macOS: drop onto the .app's Dock icon or a Finder extension; same daemon
  via virtiofs; window appears in the VM-desktop session (v1).
- This is the same UX contract the harness already automates (fetch →
  install → launch → witness): the daemon is "atlh minus the corpus".

## 5. Cost comparison vs the five walls

| | Five walls (native ports) | VM-launcher path |
|---|---|---|
| Core work | ART host port (AOSP-scale, not our code) + session-stack/ALSA/GL re-backends | packaging (.wsl, VZ app), guest daemon, glue configs |
| Ongoing cost | 3 host variants of api-impl-jni forever; gates split per-OS | one runtime; gates/witnesses valid everywhere; VM = another `runners.toml` entry |
| Fidelity risk | every re-backed subsystem (audio/GL/intents) re-learns Android semantics | byte-identical to the Linux product |
| New failure modes | language/OS integration bugs at every seam | VM availability (old/locked-down machines), RAM/disk, signing |
| Evidence | none, ever attempted | `mini` runs Andrun in a Mac VM **today**; WSLg shape documented as near-free since Nov-21 (`platform-research`/`platform-reach.md`) |

The launcher path does not make the walls irrelevant (native *feel*,
battery, ARM-Windows x86-NDK apps would eventually want them) — it defers
them behind a product that can exist first.

## 6. Risks to watch (none fatal)

- Windows: WSL unavailable/forbidden on locked-down or very old machines
  (state minimums; QEMU fallback = last resort, don't promise it); SmartScreen
  until signed; ARM Windows laptops run arm64 guests → Android apps with
  x86-only NDK libs need in-guest emulation (FEX/box64) later — dex-only
  apps (most F-Droid corpus) unaffected.
- macOS: virtio-GPU wants macOS 14+, virtio-sound macOS 15+ (corrected by the
  2026-11-23 spike; set a minimum; Intel Macs OK via x86_64 guest); RAM
  budgeting (a second browser-class VM); MAS auto-update
  friction (DMG solves).
- Guest image updates: pin an in-guest update channel from day one; never
  re-download the world on every host update.
- Licensing: unbranded rootfs (GPLv3 code fine, distro trademarks not);
  fork stays GPLv3 with source offer; user APKs are theirs.

## 7. Evidence steps before any commitment (cheap → bigger)

1. **mini audio A/B (smallest)**: fix the Lima VM sound device (virtio
  sound / Pulse backend) so `snd_pcm_hw_params` succeeds → audible music
  from Andrun **in a macOS VM** closes the only known VM-UX hole and
  validates the audio-glue pattern shared with WSLg.
2. **WSL2 spike (1 Windows box, ~day)**: tar the bazzite bootstrap rootfs →
  `wsl --import` → run the gate apps: expect WSLg windows + taskbar icons;
  probe asound.conf → Pulse for AudioTrack; run `rig atlh run` against it
  as a runner (network ssh over the VM works). A 0-regression-style gate on
  a Windows host is the demo.
3. **VZ spike on a real Mac (~day)**: `vfkit`/`vfrust` boot the same rootfs
  (EFI kernel direct boot) with virtio-gpu + virtio-sound + virtiofs;
  launch one witness app; screenshot via the VM display.
4. Only then: launcher/daemon design doc + guest-image packaging task.
  *Design doc written 2026-11-23: [../launcher-design.md](../launcher-design.md)
  (D22) — shape-only: the project is pre-release and the design's job is to
  keep exe/dmg/flatpak/appimage all open. Packaging work is gated on real
  spikes (macOS display path, WSL2) and on Andrun's own readiness.*

Reproduce the facts: Microsoft WSL docs "build-custom-distro.md" /
"use-custom-distro.md" + `wsl-config` (`guiApplications`); WSLg README
(Weston enumerates `.desktop` apps); WWDC22/WWDC26 Virtualization
sessions (Linux guests, virtio GPU/sound); Docker Desktop Mac feedback
issue (their guest kernel `CONFIG_SOUND` off — a guest-config choice, not
a VZ limit); Lima/UTM/Colima/oriel as shipping precedents.

## 8. Spike results (2026-11-23, runner: mini)

Steps 1 and 3 ran on the Mac mini (`atl-mini`); step 2 (WSL2) stays a todo
(owner has no Windows box set up yet). Verdict: **both macOS-side evidence
steps pass** — the exe/DMG model is no longer hypothetical on macOS.

Step 3 — VZ boot (PASS). The Lima guest disk (`~/.lima/atl/disk`, raw) was
cloned to `~/andrun-vz/disk.raw` and booted headless under plain Virtualization.
framework — first `vfkit` (EFI, `--device virtio-blk/net/rng/serial`), then our
own minimal Swift launcher — with **no Lima involved**. `atlh run --run-id vzs1
--pkg org.fossify.math`: ALIVE, `interact_ok=1`, `ExpectText("15")` found. The
same guest image works under a hand-written VZ launcher; the Lima dependency is
convenience, not a requirement.

Audio A/B (PASS, with a corrected root cause). The old "mini has no audio"
story was wrong in detail: **Lima attaches no audio device to the guest at all**
(`/proc/asound` absent inside `atl`), so AudioTrack's `snd_pcm_open` failed —
a Lima config gap, not a Mac mini hardware limit (the mini has no display
attached, but that is unrelated). Two more guest-side gaps found on the way:
the cloud image lacked `kernel-modules` (no `virtio_snd.ko` for the running
kernel) and PipeWire/access setup. With a virtio-sound device attached:
`virtio_snd` binds, `/proc/asound/cards` shows `VirtIO SoundCard`, PipeWire
lists "Virtio 1.0 sound" as a sink, `aplay -D default` exits 0, and Fossify
Musicplayer's AudioTrack opens with **no `snd_pcm_hw_params` error** (earlier
runs failed there). `AudioTrack.flush()` remains a fork stub (unrelated to VM
audio). Not yet verified: audible output at the host speakers (VZ feeds the
CoreAudio default device; headless mini, nobody listened).

Key practical facts paid for:
- **vfkit v0.6.4 has no sound device at all** — the product launcher must use
  `VZVirtioSoundDeviceConfiguration` directly (macOS 15+; corrects the macOS 14
  guess in §6). A ~60-line Swift launcher suffices:
  `scripts/research/vzsnd.swift` (build: `xcrun swiftc -O`, ad-hoc
  `codesign -s - --entitlements` with `com.apple.security.virtualization`).
- macOS 26.4 SDK API idioms (property-setter style for EFI boot loader/
  variable store/NAT attachment; `serialPorts` + `VZFileHandleSerialPortAttachment`;
  `Result`-based `vm.start`) are baked into the spike file.
- Guest image for the product must ship `kernel-modules` (virtio_snd), enable
  PipeWire (linger or session autostart), and note that audio-group grants need
  a user-manager restart to take effect.
- The sandbox is not an audio blocker: bwrap-mapped-root opens the PCM node fine.

Spike artifacts live on `atl-mini:~/andrun-vz/` (`disk.raw`, `vzsnd`, logs) and
in `scripts/research/`. One vfkit/VZ process per machine at a time, and the
spike VM (4 GiB) plus `atl` (12 GiB) do not fit the 16 GB mini together.
