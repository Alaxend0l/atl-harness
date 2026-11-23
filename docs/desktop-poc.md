<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Desktop PoC: ATL apps as windows on the workstation

Goal: run a real Android app (Gramophone, `org.akanework.gramophone`) as a
normal window on the workstation's Wayland session - "apps as windows on a
Linux desktop" - without the harness, on the laptop itself. Proven
2026-11-23 on the Fedora 45 laptop (noctalia compositor, Wayland, PipeWire)
with fork tip `a52665e6`: Gramophone opened a native Wayland window, scanned
the whole `~/Music` library through ATL's MediaStore (4852 files probed),
resumed playback mid-track and played through the laptop speakers via the
ALSA→PipeWire plugin.

## Why a container

The laptop's sudo needs a password (agent has none), so the whole toolchain
lives in a **rootless podman container** - I'm root inside, the host is
untouched. Everything ATL needs (GTK4, EGL, PipeWire, Wayland) passes
through mounts, exactly like distrobox would.

## Recipe

```sh
# 1. Build chain once, inside the container (host packages NOT needed):
podman pull docker.io/library/fedora:44
podman create --name atl-desktop \
  --security-opt label=disable --network=host \
  --device /dev/dri --group-add keep-groups --shm-size=2g \
  -v "$XDG_RUNTIME_DIR:$XDG_RUNTIME_DIR" \      # wayland-0 + pipewire-0 sockets
  -v "$HOME/Develop/atl-desktop-poc:/src/atl" \ # worktree at the fork tip
  -v atl-desktop-src:/root/src \                # named volume: sources+builds
  -v "$HOME/Music:/data/music:ro" \
  -v "$HOME/atl-desktop-poc/apks:/root/apks:ro" \
  -e WAYLAND_DISPLAY -e XDG_RUNTIME_DIR \
  docker.io/library/fedora:44 sleep infinity
podman start atl-desktop
podman exec atl-desktop dnf install -y sudo git procps-ng   # bootstrap.sh uses sudo; pkill for restarts
podman cp scripts/runner atl-desktop:/src/harness/scripts/runner
podman exec atl-desktop bash -c 'cd /root/src && git clone --branch v5.8.2-stable --depth 1 \
  https://github.com/wolfSSL/wolfssl.git wolfssl && \
  for r in bionic_translation art_standalone libopensles-standalone; do \
    git clone https://gitlab.com/android_translation_layer/$r.git; done'
podman exec -d atl-desktop bash -c 'export HARNESS_DIR=/src/harness ATL_SRC=/src/atl \
  STEPS="packages wolfssl bionic art opensles atl"; bash /src/harness/scripts/runner/bootstrap.sh'
# (~15 min on 32 cores; markers in /root/src/.bootstrap/, logs in /root/src/logs/)

# 2. Freeze the toolchain image, then run apps:
podman commit atl-desktop atl-desktop-poc:latest   # after stopping it
# (recreate with `podman create` from that image; named volumes persist)

ANDROID_APP_DATA_DIR=/root/atl-data ATL_MEDIA_DIRS=/data/music \
  GDK_BACKEND=wayland android-translation-layer /root/apks/<app>.apk
```

Notes learned on the way:

- `ANDROID_APP_DATA_DIR` must **exist** before the run (ATL doesn't mkdir the
  root, only the app subdir).
- `--security-opt label=disable` for the socket/device bind-mounts (host
  SELinux is enforcing); `--group-add keep-groups` keeps the `render` group
  for `/dev/dri`.
- `ATL_MEDIA_DIRS` (absolute, inside the container) is what feeds ATL's
  MediaStore; the harness passes the same variable (docs/usage.md).
- Wayland: GTK4 does the rest; no Xvfb, no Xwayland needed.
- Audio: install `pipewire-alsa` in the container (it's in
  `scripts/runner/packages.txt`) and mount `$XDG_RUNTIME_DIR`; ALSA's
  `default` PCM is then the host's PipeWire.
- Media keys on Wayland work through the same capture-phase window
  controller as on X11 - **witnessed 2026-11-23**: `atlDispatchMediaKey`
  lines for PLAY_PAUSE (resumed at 59 s) and NEXT (origKey=87,
  handled=true -> flush/release/new `AudioTrack.play()`, next track
  playing, position advancing), and the resume fix holds on the desktop.
  The catch is the **compositor**, not ATL: umbriel (the owner's session
  runs `umbriel` wrapping `noctalia`) binds `XF86AudioPlay/Next/Prev` to
  `noctalia msg media ...` (MPRIS) by default, so the keys never reach
  any client. Commenting those three binds in
  `~/.config/umbriel/config.toml` makes them flow to the focused window.
  A temporary `g_printerr` probe in the C controller proved the routing
  (typed keys fired the capture controller, media keys did not - until
  the binds were off).

## What this does NOT change

Runners and gates stay as documented (setup.md, rig.md): the PoC never
touches the host and is not a runner. It *diagnosed* fork bugs though -
the boot auto-play/"skipping" complaint was root-caused here (`wpctl
status` caught an `[active]` PipeWire stream with zero
`AudioTrack.play()` calls) and fixed in this worktree (`8faa3ec3` on
`local-fixes`, see docs/agent-tasks/audiotrack.md). The container now
runs that build; the image `atl-desktop-poc:latest` predates it.
