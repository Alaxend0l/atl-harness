#!/usr/bin/env bash
# Build ATL and its dependencies on a Fedora runner (host or distrobox).
# Run through `rig bootstrap`; it passes these environment variables:
#   HARNESS_DIR  harness checkout (absolute)        MAKE_JOBS  parallel jobs
#   ATL_SRC      ATL checkout (absolute)            ATL_GIT    ATL fork clone URL
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
ATL_GIT=${ATL_GIT:-git@github-atl-fork:Alaxend0l/android-translation-layer.git}
ATL_UPSTREAM=https://gitlab.com/android_translation_layer/android_translation_layer.git
ALL_STEPS="packages rust sources wolfssl bionic vixl art opensles atl harness microg"
STEPS=${STEPS:-$ALL_STEPS}
GITLAB=https://gitlab.com/android_translation_layer

export PKG_CONFIG_PATH=/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}
# shellcheck disable=SC1090
[ -f ~/.cargo/env ] && . ~/.cargo/env

log() { echo "[$(date +%T)] $*"; }

step_packages() {
  command -v dnf >/dev/null || { echo "only Fedora (dnf) is supported" >&2; return 1; }
  # shellcheck disable=SC2046 # one package per word
  sudo dnf install -y $(grep -v '^#' "$(dirname "$0")/packages.txt")
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
  # The fork is private: the runner needs the github-atl-fork ssh alias and a
  # deploy key (docs/rig.md). Upstream is added fetch-only (decisions D1, D13).
  clone "$ATL_GIT" "$ATL_SRC" --branch main
  git -C "$ATL_SRC" remote get-url upstream >/dev/null 2>&1 \
    || git -C "$ATL_SRC" remote add upstream "$ATL_UPSTREAM"
  git -C "$ATL_SRC" remote set-url --push upstream DISABLED-hard-fork-never-push-upstream
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

# microG GmsCore from microG's F-Droid repo, installed into an ATL data dir
# template (~/src/atl-data/microg) with ATL's own --install-internal. Runs use
# it with `atlh run --data-template ~/src/atl-data/microg` (docs/usage.md).
MICROG_REPO=https://microg.org/fdroid/repo
step_microg() {
  local dir=$SRC/microg tpl=$SRC/atl-data/microg name sha ver
  mkdir -p "$dir" "$SRC/atl-data"
  cd "$dir"
  curl -fsSL -o index-v2.json "$MICROG_REPO/index-v2.json"
  read -r name sha ver < <(python3 -I -c '
import json
p = json.load(open("index-v2.json"))["packages"]["com.google.android.gms"]
v = max(p["versions"].values(), key=lambda v: v["manifest"]["versionCode"])
print(v["file"]["name"].lstrip("/"), v["file"]["sha256"], v["manifest"]["versionName"])')
  [ -f "$name" ] || curl -fsSL -o "$name.part" "$MICROG_REPO/$name"
  [ -f "$name.part" ] && mv "$name.part" "$name"
  echo "$sha  $name" | sha256sum -c -
  # ATL names the installed copy after the package, so the file name matters.
  cp "$name" com.google.android.gms.apk

  # Globals, not locals: the EXIT trap runs after this function has returned.
  local disp
  MG_T=$(mktemp -d)
  local t=$MG_T
  mkdir -p "$t/data" "$t/xdg" "$t/cache"
  Xvfb -displayfd 4 -screen 0 1080x1920x24 -nolisten tcp 4>"$t/display" 2>/dev/null &
  MG_XP=$!
  trap 'kill "$MG_XP" 2>/dev/null || true; rm -rf "$MG_T"' EXIT
  for _ in $(seq 100); do [ -s "$t/display" ] && break; sleep 0.1; done
  disp=":$(head -1 "$t/display")"
  env -u WAYLAND_DISPLAY ANDROID_APP_DATA_DIR="$t/data" HOME="$t" XDG_DATA_HOME="$t/xdg" \
    XDG_CACHE_HOME="$t/cache" DISPLAY="$disp" GDK_BACKEND=x11 GDK_DISABLE=glx \
    timeout 300 android-translation-layer --install-internal "$dir/com.google.android.gms.apk"
  test -f "$t/data/_installed_apks_/com.google.android.gms.apk"
  # Read-only APK and native libs: runs hard-link instead of copying ~180 MB.
  find "$t/data/_installed_apks_" "$t/data"/*_/lib -type f -exec chmod a-w {} +
  echo "microG GmsCore $ver ($name, sha256 $sha)" > "$t/data/MICROG_VERSION"
  rm -rf "$tpl.new"
  cp -a "$t/data" "$tpl.new"
  rm -rf "$tpl"
  mv "$tpl.new" "$tpl"
  du -sh "$tpl"
  cat "$tpl/MICROG_VERSION"
}

step_harness() {
  # Steps run in separate subshells: pick up a Rust installed by step_rust.
  # shellcheck disable=SC1090
  . ~/.cargo/env
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
