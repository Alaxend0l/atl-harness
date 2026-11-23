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
