# Setup

## Machine roles

| Machine | Role | Needs |
|---|---|---|
| **Workstation** (Linux) | Holds the git clone; you edit, commit and drive the box from here. | git, ssh, rsync; optionally Rust to run `cargo test` locally |
| **Runner box** (Linux, on the LAN, reachable over SSH) | Builds and runs ATL; runs the harness and later the coding agent. Stores the corpus, logs and database. | Rust, Xvfb, ImageMagick, Mesa, the ATL fork built from source |

You can run everything on one machine too: the harness doesn't care, and
`scripts/rig` ([rig.md](rig.md)) is the convenience layer for multiple machines.

## Workstation

```sh
git clone git@github.com:Alaxend0l/atl-harness.git
cd atl-harness
cargo test          # optional; unit tests run without Xvfb/ATL
```

Set up SSH to the box with a key and an alias in `~/.ssh/config`:

```
Host atl-box
    HostName <box-ip-or-hostname>
    User <you>
```

Then register it with `scripts/rig add` ([rig.md](rig.md#adding-a-runner)),
which also covers building everything (`rig bootstrap`). The rest of this
file documents what bootstrap does and why.

Legacy single-runner helper (deprecated), e.g. in your shell profile:

```sh
export ATLH_REMOTE=atl-box
export ATLH_REMOTE_BOX=atl             # distrobox to run in; omit if none
# export ATLH_REMOTE_DIR=atl-harness   # remote dir, relative to ~ (default)
```
(fish: `set -Ux ATLH_REMOTE atl-box` etc.)

`scripts/remote.sh sync` copies your working tree (including uncommitted
changes, excluding `work/`, `target/`, `.git/`) to the box. If you'd rather
keep the box on git, clone the repo there instead and `git pull`; both work.

## Runner box

The current box runs Bazzite, an immutable Fedora system, so everything
lives in a Fedora 44 distrobox named `atl` (see decisions D10). The
distrobox shares `$HOME` with the host, so `~/atl-harness`, `~/.cargo` and
the ATL checkout are visible from both, but build and run inside the box.

```sh
distrobox create --name atl --image registry.fedoraproject.org/fedora-toolbox:44
distrobox enter atl
sudo dnf install gcc xorg-x11-server-Xvfb ImageMagick mesa-dri-drivers glx-utils rsync tmux git
```

On a regular distro, skip the distrobox and install directly:

### System packages
- Debian/Ubuntu: `sudo apt install build-essential xvfb imagemagick mesa-utils rsync tmux`
- Fedora: `sudo dnf install gcc xorg-x11-server-Xvfb ImageMagick mesa-dri-drivers glx-utils rsync tmux`
- Arch: `sudo pacman -S base-devel xorg-server-xvfb imagemagick mesa rsync tmux`

### Rust
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
SQLite is bundled into the binary, so no `libsqlite3-dev` is needed.

### ATL
Upstream's `doc/Build.md` is the reference. This is what it took on Fedora 44
(inside the `atl` distrobox, 2026-11-21). Everything installs to
`/usr/local`; sources live in `~/src`. Once the fork repository exists, build
ATL itself from the fork instead.

```sh
sudo dnf install java-25-openjdk-devel meson ninja-build autoconf automake libtool \
  make gcc-c++ python3 zip unzip maven objectweb-asm valgrind-devel libicu-devel \
  openssl-devel expat-devel xz-devel wayland-protocols-devel wayland-devel \
  gtk4-layer-shell-devel \
  'pkgconfig(gtk4)' 'pkgconfig(libbsd)' 'pkgconfig(libportal)' 'pkgconfig(sqlite3)' \
  'pkgconfig(libwebp)' 'pkgconfig(liblz4)' 'pkgconfig(openxr)' 'pkgconfig(webkitgtk-6.0)' \
  'pkgconfig(libsecret-1)' 'pkgconfig(egl)' 'pkgconfig(libelf)' 'pkgconfig(libunwind)' \
  'pkgconfig(alsa)' 'pkgconfig(libavcodec)' 'pkgconfig(libswscale)' 'pkgconfig(gudev-1.0)' \
  'pkgconfig(libdrm)' 'pkgconfig(libcap)' 'pkgconfig(vulkan)' 'pkgconfig(sdl2)' 'pkgconfig(sndfile)'

# /usr/local is not on Fedora's default linker / pkg-config paths
printf '/usr/local/lib64\n/usr/local/lib\n' | sudo tee /etc/ld.so.conf.d/local.conf
echo 'export PKG_CONFIG_PATH=/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig' \
  | sudo tee /etc/profile.d/zz-local-pkgconfig.sh
```

Then, in order (`sudo ldconfig` after each install):

| Component | Build | Notes |
|---|---|---|
| wolfSSL `v5.8.2-stable` | `autoreconf -i && ./configure --prefix=/usr/local CFLAGS="-O2 -Wno-error=stringop-truncation" <flags from Build.md> && make && sudo make install` | Use ATL's flag list; art_standalone's README lists flags 5.8.2 rejects. GCC 16 trips `-Werror` without the CFLAGS. |
| bionic_translation | `meson setup builddir --prefix=/usr/local && meson compile -C builddir && sudo meson install -C builddir` | Installs no `.pc` file; that's fine. |
| vixl 8.0.0 (aarch64 only) | tarball + Alpine's meson patch (`scripts/runner/patches/`), meson install | ART's ARM code generators include `vixl/aarch64/*.h` and link `-lvixl`; x86 builds skip that code. |
| art_standalone | `make -j16 ____LIBDIR=lib64 && sudo make ____LIBDIR=lib64 install` | ~25 min on 16 cores. Java 25 works although upstream says 17. Run it in tmux. |
| libopensles-standalone | meson, as above | Optional; needs SDL2 and libsndfile. |
| android_translation_layer | `meson setup builddir --prefix=/usr/local -Ddownload_tool_deps=true`, compile, install | Fedora needs `download_tool_deps` (no java-getopt package). |

Test APKs: `git clone https://gitlab.com/android_translation_layer/atl_test_apks`.

### Smoke test
```sh
# on the workstation
scripts/rig doctor
scripts/rig smoke src/atl_test_apks/org.happysanta.gd_29.apk
```

## Second runner: Mac mini (aarch64 Lima VM)

See decisions D11. On the mini (Homebrew + Lima installed):
```sh
limactl create --name atl --vm-type vz --cpus 8 --memory 12 --disk 120 \
  --mount-none template:fedora-44
limactl edit --set '.ssh.localPort = 60022' atl   # stable port for ProxyJump
limactl start atl
# optional, so the VM comes back after a reboot (needs a logged-in user):
limactl start-at-login atl
```
Add the workstation's public key to `~/.ssh/authorized_keys` inside the VM
(`limactl shell atl`), then on the workstation:
```
Host atl-mini-vm
    HostName 127.0.0.1
    Port 60022
    User <you>
    ProxyJump atl-mini
    HostKeyAlias atl-mini-vm
```
The VM is plain (mutable) Fedora, so no distrobox: install the packages
and build ATL directly, same recipe as above (`make -j6` for art_standalone;
12 GB RAM). Use `config.aarch64.toml`:
```sh
scripts/rig -r mini atlh ...     # runners.toml points mini at config.aarch64.toml
```

## Third runner: OnePlus 12 (aarch64, bwrap rootfs)

The phone runs Fedora 45 from `fedora-atomic-phone` (decisions D14). Its
root partition is 8 GiB and nearly full, and the system belongs to that
project, so the runner lives entirely in `/home/phone`: a rootfs built on
the mini (`rig -r oneplus12 rootfs --from mini`) used through `bwrap`. Reached
over USB networking (`172.16.42.1`, user `phone`, ssh alias `atl-oneplus12`
with `HostKeyAlias oneplus12`). Follow the phone project's safety rules: one
actor on the phone, nothing outside `$HOME`.

## First-run checklist

Everything here was written before ATL was run on the box, so these
assumptions need confirming. Tick them off here as you go.

- [x] **CPU architecture.** `uname -m` on the box. `aarch64` →
      `allowed_abis = ["arm64-v8a"]`; `x86_64` → `["x86_64"]` and expect
      fewer eligible apps with native code (most ship ARM only).
      *Current box: x86_64, set in `config.toml`.*
- [x] **ATL command line.** Find the binary name and flags of your build and
      set `[atl].command`. Check whether some apps need an explicit launch
      activity flag. *`android-translation-layer {apk} -w 1080 -h 1920`;
      without `-l` ATL launches the manifest's launcher activity.*
- [x] **ATL renders under Xvfb.** *Works with software GL (Mesa warns
      about DRI3; harmless). Needs `GDK_DISABLE=glx` (upstream's
      `GDK_DEBUG=gl-egl` is gone in GTK 4.22). ATL always draws a 50px
      title bar, hence `[display].crop_top_px = 50`.* Run one known-good app by hand:
      ```sh
      Xvfb :50 -screen 0 1080x1920x24 &
      DISPLAY=:50 GDK_BACKEND=x11 <atl command> some.apk
      import -display :50 -window root /tmp/shot.png
      ```
      If it fails on GL, try `LIBGL_ALWAYS_SOFTWARE=1` (add to `[atl.env]`).
      If ATL needs Wayland, the runner needs a headless compositor backend
      instead (`weston --backend=headless`); see roadmap.
- [ ] **Stub log format.** Read a few logs in `work/runs/<id>/` and update
      `[classify].stub_patterns` to match what ATL prints for unimplemented
      methods.
- [ ] **Classifier coverage.** After a 10–20 app run, `rank` prints
      "broken with no signature". Read those apps' logs and add patterns
      or classifier rules until that number is low.
- [ ] **Blank threshold.** Compare `screen_stddev` of visibly blank vs.
      working screenshots and adjust `blank_stddev_threshold`.
- [ ] **Agent CLI flags** (only when starting the agent loop): confirm the
      non-interactive invocation of opencode / claude and set `[agent]`.
