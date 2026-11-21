#!/usr/bin/env bash
# Build ATL and its dependencies on a Fedora runner (host or distrobox).
# Run through `rig bootstrap`; it passes these environment variables:
#   HARNESS_DIR  harness checkout (absolute)        MAKE_JOBS  parallel jobs
#   ATL_SRC      ATL checkout (absolute)            ATL_GIT    ATL clone URL
#   STEPS        steps to run (space separated)     FORCE      1 = redo done steps
# Each step leaves a marker in ~/src/.bootstrap/ and is skipped next time.
# Recipe and rationale: docs/setup.md.
set -euo pipefail

SRC=~/src
LOGS=$SRC/logs
MARK=$SRC/.bootstrap
mkdir -p "$SRC" "$LOGS" "$MARK"
MAKE_JOBS=${MAKE_JOBS:-$(nproc)}
ATL_SRC=${ATL_SRC:-$SRC/android_translation_layer}
ATL_GIT=${ATL_GIT:-https://gitlab.com/android_translation_layer/android_translation_layer.git}
ALL_STEPS="packages rust sources wolfssl bionic vixl art opensles atl harness"
STEPS=${STEPS:-$ALL_STEPS}
GITLAB=https://gitlab.com/android_translation_layer

export PKG_CONFIG_PATH=/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}
# shellcheck disable=SC1090
[ -f ~/.cargo/env ] && . ~/.cargo/env

log() { echo "[$(date +%T)] $*"; }

step_packages() {
  command -v dnf >/dev/null || { echo "only Fedora (dnf) is supported" >&2; return 1; }
  sudo dnf install -y \
    patch gcc gcc-c++ make autoconf automake libtool meson ninja-build python3 zip unzip git rsync tmux sqlite \
    xorg-x11-server-Xvfb ImageMagick mesa-dri-drivers glx-utils \
    java-25-openjdk-devel maven objectweb-asm \
    valgrind-devel libicu-devel openssl-devel expat-devel xz-devel \
    wayland-protocols-devel wayland-devel gtk4-layer-shell-devel \
    'pkgconfig(gtk4)' 'pkgconfig(libbsd)' 'pkgconfig(libportal)' 'pkgconfig(sqlite3)' \
    'pkgconfig(libwebp)' 'pkgconfig(liblz4)' 'pkgconfig(openxr)' 'pkgconfig(webkitgtk-6.0)' \
    'pkgconfig(libsecret-1)' 'pkgconfig(egl)' 'pkgconfig(libelf)' 'pkgconfig(libunwind)' \
    'pkgconfig(alsa)' 'pkgconfig(libavcodec)' 'pkgconfig(libswscale)' 'pkgconfig(gudev-1.0)' \
    'pkgconfig(libdrm)' 'pkgconfig(libcap)' 'pkgconfig(vulkan)' 'pkgconfig(sdl2)' 'pkgconfig(sndfile)'
  printf '/usr/local/lib64\n/usr/local/lib\n' | sudo tee /etc/ld.so.conf.d/local.conf >/dev/null
  echo 'export PKG_CONFIG_PATH=/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}' \
    | sudo tee /etc/profile.d/zz-local-pkgconfig.sh >/dev/null
}

step_rust() {
  if [ ! -x ~/.cargo/bin/cargo ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c clippy,rustfmt
  fi
  # shellcheck disable=SC1090
  . ~/.cargo/env
  cargo --version
}

clone() { # url dir [git clone args...]
  local url=$1 dir=$2; shift 2
  if [ -d "$dir/.git" ]; then echo "have $dir"; else git clone "$@" "$url" "$dir"; fi
}

step_sources() {
  clone https://github.com/wolfSSL/wolfssl.git "$SRC/wolfssl" --branch v5.8.2-stable --depth 1
  for r in bionic_translation art_standalone libopensles-standalone atl_test_apks; do
    clone "$GITLAB/$r.git" "$SRC/$r"
  done
  clone "$ATL_GIT" "$ATL_SRC"
}

meson_install() { # dir [meson setup args...]
  local dir=$1; shift
  cd "$dir"
  if [ -d builddir ]; then
    meson setup --reconfigure builddir --prefix=/usr/local "$@"
  else
    meson setup builddir --prefix=/usr/local "$@"
  fi
  meson compile -C builddir
  sudo meson install -C builddir
  sudo ldconfig
}

step_wolfssl() {
  cd "$SRC/wolfssl"
  autoreconf -i
  # Flags from ATL's doc/Build.md. GCC >= 16 trips -Werror on stringop-truncation.
  ./configure --prefix=/usr/local CFLAGS="-O2 -Wno-error=stringop-truncation" \
    --enable-shared --disable-opensslall --disable-opensslextra --enable-aescbc-length-checks \
    --enable-curve25519 --enable-ed25519 --enable-ed25519-stream --enable-oldtls --enable-base64encode \
    --enable-tlsx --enable-scrypt --disable-examples --enable-crl --enable-jni --enable-sessioncerts
  make -j"$MAKE_JOBS"
  sudo make install
  sudo ldconfig
}

step_bionic() { meson_install "$SRC/bionic_translation"; }

# ART's ARM code generators need vixl (aarch64 hosts only; x86 builds skip that
# code). Version and meson patch follow Alpine's testing/vixl package.
VIXL_VER=8.0.0
VIXL_SHA512=9747166c808d1406fbd3d3cfe9a584b5837df368249c6f63acb754602c7ad7e70e668b0f21c1d917b100d653bd76172fbb5940685be1f38742f8a9a02f0d962c
step_vixl() {
  case "$(uname -m)" in
    aarch64|armv7*) ;;
    *) echo "not needed on $(uname -m)"; return 0 ;;
  esac
  cd "$SRC"
  [ -f "vixl-$VIXL_VER.tar.gz" ] || curl -fsSL -o "vixl-$VIXL_VER.tar.gz" \
    "https://github.com/Linaro/vixl/archive/refs/tags/$VIXL_VER.tar.gz"
  echo "$VIXL_SHA512  vixl-$VIXL_VER.tar.gz" | sha512sum -c -
  rm -rf "vixl-$VIXL_VER"
  tar -xzf "vixl-$VIXL_VER.tar.gz"
  patch -d "vixl-$VIXL_VER" -p1 < "$HARNESS_DIR/scripts/runner/patches/vixl-0001-meson-support.patch"
  meson_install "$SRC/vixl-$VIXL_VER"
  ls /usr/local/include/vixl/aarch64/disasm-aarch64.h
}

step_art() {
  cd "$SRC/art_standalone"
  make -j"$MAKE_JOBS" ____LIBDIR=lib64
  sudo make ____LIBDIR=lib64 install
  sudo ldconfig
  pkg-config --modversion art-standalone
}

step_opensles() { meson_install "$SRC/libopensles-standalone"; }

step_atl() {
  meson_install "$ATL_SRC" -Ddownload_tool_deps=true
  command -v android-translation-layer
}

step_harness() {
  cd "$HARNESS_DIR"
  cargo build --release
}

for s in $STEPS; do
  case " $ALL_STEPS " in *" $s "*) ;; *) echo "unknown step '$s' (have: $ALL_STEPS)" >&2; exit 2 ;; esac
done
for s in $STEPS; do
  if [ -e "$MARK/$s.done" ] && [ "${FORCE:-0}" != 1 ]; then
    log "skip $s (done; FORCE=1 to redo)"
    continue
  fi
  log "step $s -> $LOGS/bootstrap-$s.log"
  # Not `if ( ... )`: bash disables set -e inside an if condition, so a
  # failing command mid-step would be ignored.
  set +e
  ( set -e; "step_$s" ) > "$LOGS/bootstrap-$s.log" 2>&1
  rc=$?
  set -e
  if [ "$rc" = 0 ]; then
    touch "$MARK/$s.done"
    log "ok $s"
  else
    log "FAILED $s; last lines of $LOGS/bootstrap-$s.log:"
    tail -n 25 "$LOGS/bootstrap-$s.log"
    exit 1
  fi
done
log "bootstrap complete"
