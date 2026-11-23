<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Is GTK4 the right toolkit for Andrun? (owner question 2026-11-23 — research only, no decision executed)

Question: if Andrun's goal is running APKs "on any platform", does the
graphics library matter at all, and is GTK4 the best option for that goal?
Companion to `platform-reach.md` (cross-OS walls) and `rust-rewrite.md`
(§10 transition mechanics). Per the research-task rule: report, don't adopt.

## TL;DR

- "The graphics library" is really **three layers** in this codebase:
  (a) compositing/drawing (GSK + EGL/GLES), (b) the **host widget kit**
  (window, input, IME, popovers/menus, text widgets, theming, dialogs),
  (c) OS session services (DBus, portals, MPRIS, libsecret, gudev,
  ALSA, Wayland extensions).
- **APK compatibility is essentially independent of all three**: apps only
  ever see the dex-visible Java tier. Measured defect history has zero
  "GTK caused it" bugs. Swapping toolkits would not raise the compatibility
  rate by even one signature.
- **GTK4 is not what blocks other operating systems.** Measured walls, in
  order of cost (see `platform-reach.md`): art_standalone's Linux-only
  host, the glibc/bionic assumptions, then the *session stack* (DBus,
  libportal, gtk4-layer-shell, gudev, libsecret, webkitgtk, ALSA). GTK4
  itself actually runs on Windows and macOS; the dependencies around it do
  not port for free.
- Keep GTK4 as the **Linux backend**. The strategic move that preserves
  every future option is a **host abstraction seam** (surface/input/IME/
  dialogs behind an interface) — expensive today, and it falls out almost
  for free if the Rust migration (D21 option B) is ever adopted, because
  every migrated file must hide its host calls anyway.
- **Windows needs no toolkit decision at all**: the WSLg path (a Linux
  guest) gives ART + GTK for free. Native macOS is an ART-host project
  first, toolkit question second.

## 1. What GTK actually does here (measured 2026-11-23, fork `local-fixes`)

`src/api-impl-jni` = 86 C files in one shared library; **54 reference GTK**:
`GtkWidget` ×200, `GtkWindow` ×29, `GtkSnapshot` ×104 snapshot call sites
(GSK is the draw path), `GtkEventController` ×18 (input), `GtkPopover`
(menus), `GtkIMContext` (IME), `GtkLabel`/`GtkText` (text), `GtkCssProvider`
(theming). Plus Wayland OSK protocols and `meson` deps: gtk4,
gtk4-layer-shell, libportal, wayland-client, gudev, libsecret, webkitgtk,
sqlite3, ffmpeg, vulkan, egl.

So GTK is **not a thin "window + GL" wrapper**: the architecture mirrors
each Android view class onto a host widget (`WrapperWidget` subclass per
view), outsourcing focus, scrolling, popup, text-editing and IME behavior
to the toolkit. That outsourcing is a large part of why 31 k lines of C
suffice where a from-scratch framework would need a widget kit of its own.
The other part — the part that decides compatibility — is the Java tier:
the app never sees GTK.

Evidence from our own bug history (all root-caused): ALSA `start_threshold`,
missing `MediaSession.sendSessionEvent`, `AlertDialog.Builder.setView`
dropped, click propagation, marquee/Compose semantics. None was toolkit
semantics; none would have been prevented by Qt/SDL/Skia. (The one true
toolkit gotcha — Cairo/GSK matrix handling, `snapshot_transform_android_matrix`
— is a documented fork-local hazard either way.)

Harness independence: element-addressed witnesses (P1) use the **fork's own
Java-side dumper**, not GTK accessibility — the test rig survives any
toolkit swap.

## 2. If GTK were replaced, what would it be replaced *with*?

| Candidate | What it gives | What it costs |
|---|---|---|
| Qt5/6 | Full widget behavior set (like GTK), cross-platform, LGPL | rewrite of all 54 files + its own event loop/a11y model; C++ (does not help the Rust question) |
| SDL3 / GLFW | Window + input + GL only, tiny, portable, permissive | **you now own the widget kit**: menus, text editing, IME, focus, scrolling, theming — the hidden budget; this is the cost GTK currently amortizes |
| Native per-OS (win32/Quartz/GTK) | Max per-OS fidelity | 3× the host layer; only rational behind a seam |
| AOSP's own UI stack (Skia/HWUI via libandroid-ui) | Highest possible rendering fidelity | porting parts of Android itself; enormous |

None changes APK compatibility by itself. The "best" choice depends
entirely on which OS you're targeting and whether an ART-host build exists
for it — i.e. the toolkit question is *downstream* of the ART-host question.

## 3. The walls to "any OS", re-measured (deps inventory above)

1. **ART host** (art_standalone is Linux-only today) — dominates the cost
   of macOS/Windows-native; nothing else matters until this moves.
2. **Session stack**: DBus + libportal (file/URI/email), MPRIS, gudev,
   libsecret, gtk4-layer-shell, Wayland protocols — Linux desktop
   integration worth keeping, replaceable per-OS behind a seam.
3. **Audio**: ALSA in 4 C files — small; CoreAudio/WASAPI backends are a
   bounded job.
4. **GPU**: EGL/GLES + GSK — on macOS via ANGLE/MoltenVK; Windows has
   native ANGLE. Not the hard part.
5. **GTK itself** — cross-platform already (win32/quartz backends); least
   of the five walls.

Conclusion: "GTK4 blocks portability" is false by measurement. "Andrun
runs on Linux" is true because of the items above, and each is
replaceable — but each is also *outside* the toolkit.

## 4. The architecture that maximizes options (if cross-OS becomes a goal)

A **host seam**, not a toolkit swap: interfaces for
`surface/window`, `input event injection`, `IME`, `dialogs/pickers`,
`clipboard`, `media output`, with GTK4 registered as the Linux backend.
Carving it out of 54 GTK-welded files today buys nothing without an ART
plan. But per D20 the fork is free to restructure, and if D21-B (C→Rust)
is adopted, **the seam should be introduced as part of the migration**:
each file migrating to Rust must hide its host calls behind a backend
interface anyway, and Rust's FFI story toward SDL3/Qt/native targets is
better than C's. Option D's spike can rehearse this: do one toolkit-light
component (audio state machine) and note how tangled the GTK-heavy one
(element dump widget traversal) would be.

Cheapest honest cross-OS ranking (unchanged from `platform-reach.md`):
Linux (done) → Windows via WSLg (no code decision needed) → Android-as-host
(the phone runner already proves this shape) → macOS native (ART-host
project, then ANGLE + CoreAudio, then possibly a non-GTK backend).

## 5. What would change this analysis

- A sustained defect class rooted in GTK semantics (today: zero observed;
  the GSK matrix incident is one documented gotcha, manageable).
- Real user demand for *native* Windows/macOS windows (WSLg-quality
  demand may be "good enough" and cost zero).
- ART gaining a Darwin/Windows host build (changes wall #1's cost entirely).
- Adopting D21-B: then the seam gets built per-migrated-file as a side
  effect, and the toolkit becomes a pluggable backend decision, not a
  bet-the-fork one.

Reproduce the measurements: `grep -rl "gtk_\|Gtk" src/api-impl-jni | wc -l`,
`grep -rhoP "Gtk[A-Za-z]+" src/api-impl-jni | sort | uniq -c | sort -rn`,
and the `dependency(` lines in `meson.build`.
