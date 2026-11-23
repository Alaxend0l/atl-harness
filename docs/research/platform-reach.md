<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Platform reach: Andrun (ATL fork) beyond phones (owner question, 2026-11-23)

Parked note — analysis only, explicitly **not** scheduled work.

## TL;DR
- **Linux desktop: already the deployment mode.** ATL is a GTK4 program;
  each app is already a window (owner desktop `atl-app` → `atl-sandbox`
  chain on the phone; distrobox on bazzite; Xvfb in the harness). The
  gaps to a usable desktop are *ecosystem* gaps, not window-system ones:
  cross-app intents/ActivityManager (logs literally say "no session bus
  in ATL"), media-session routing (see P0, element-addressed-interaction
  doc), notifications bridged to the desktop, permission dialogs, file
  pickers, density/scale for phone-sized layouts, IME polish. All of
  these are depth-of-emulation items — exactly what the corpus queue
  already ranks.
- **Windows: WSLg path, largely works "for free".** WSL2 = Linux
  userspace + WSLg Wayland/X11 passthrough; an Andrun rootfs with
  per-app windows would be expected to run near-as-is. Native Windows
  falls under the macOS cost analysis below.
- **macOS: feasible, but a real porting project** with three
  load-bearing walls:
  1. **art_standalone is Linux-only today** (AOSP's Darwin host target was
     dropped years ago; the fork's `bionic_compat.c` already papers over
     some of this for Linux). An ART-host-on-mac decision dominates the
     entire cost.
  2. **Audio**: `AudioTrack` is ALSA (`snd_pcm_*`) — needs a CoreAudio
     backend or a thin ALSA shim.
  3. **GPU**: the fork's GL/EGL path (plus llvmpipe fallback, see the
     `gdk_gl_context_make_current` warnings in runs) would want
     ANGLE/MoltenVK on macOS since GL is deprecated there.
- **Why this is credible at all** — architectural differentiator vs
  Waydroid/BlueStacks-type layers: **no Android kernel dependency**
  (no binder, no ashmem, no kernel modules). art_standalone executes dex
  in an ordinary userspace process; that is why the fork already runs in
  a distrobox, a bwrap rootfs on the phone, under Xvfb, and on aarch64.
  Portable-by-construction is the asset to protect.
- **Legal**: GPLv3 throughout (D19 rename applies); nothing goes upstream
  either way (D1).

## If ever picked up, ordering
1. Linux desktop UX (cross-app intents first — biggest missing glue).
2. WSLg as the "Windows support" checkbox.
3. macOS only after an explicit ART-host decision.

## macOS without touching our code: the VM/container layer (owner question, 2026-11-23)

The five walls (see graphics-backend.md §3) all vanish inside a Linux
guest — and Andrun needs nothing special from a guest, because it has no
Android-kernel dependency: `art_standalone` is a userspace process, which
is why it already runs in a distrobox, a bwrap rootfs on the phone, under
Xvfb, and **in a macOS VM today: the `mini` runner is a Fedora aarch64
Lima VM on the M4 Mac mini running Andrun apps**. Per-app bwrap sandboxes
work there too (real Linux kernel = namespaces).

macOS-side software that makes Andrun usable, none requiring changes on
our end:
- **Lima** (what mini uses): headless-ish VMs, file sharing, ssh; X11
  forwarding to an X server on the Mac gives *per-app windows*
  (each `atl-app` window on the Mac desktop).
- **XQuartz / Wayng**: the host-side X servers for that per-app-window UX.
- **UTM**: GUI front-end to QEMU; full Linux desktop in one Mac window
  with virtio display, CoreAudio-backed sound, clipboard, shared folders.
- **Apple's own stack**: Virtualization.framework GUI Linux (macOS 13+,
  SPICE agent clipboard/drag), `apple/container` (free CLI; community
  recipes do X11 forwarding out of it), Krunkit (guest GPU via Mesa
  Venus — the interesting one for GL performance), MacVHPT. Docker
  Desktop/OrbStack/podman are the same VZ substrate; Andrun fits
  containers (no kernel modules), GUI via XQuartz, audio needs a
  PipeWire/Pulse socket into the container.

Known gaps for a Mac UX demo (all outside Andrun's code):
1. mini's Lima sound device fails `snd_pcm_hw_params` (HANDOFF) — a VM
   config item (VirtioSound/pulse backend), worth an A/B: audible audio
   in a VM would close the last UX hole.
2. GL inside VMs is virtio (Venus/VirGL) or llvmpipe — mini runs today;
   Krunkit-class Venus is the upgrade path.
3. Polish: no native macOS titlebars/shortcuts in VM-desktop mode; X11
   forwarding is the closest; host IME edge cases; x86-only guests are
   slow (irrelevant — our builds are aarch64 on Apple Silicon).

Packaging is mechanical: `scripts/runner/bootstrap.sh` already builds the
rootfs this VM needs; `runners.toml` + `scripts/rig` already treat remote
Linux boxes as runners — a Mac user's VM is just another entry.
