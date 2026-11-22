#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Build one component on a runner, log it, and summarise errors.
# Usage (normally via `rig build`): build.sh TARGET
#   TARGET: atl | art | bionic | opensles | harness
# Env: ATL_SRC (atl source dir), HARNESS_DIR, MAKE_JOBS, INSTALL=1|0, CLEAN=1|0
# Logs: ~/src/logs/build-TARGET-<time>.log (+ -latest.log/-latest.json).
# Prints "BUILD OK ..." or "BUILD FAILED ..." plus parsed errors; exit status
# is the build's. After installing ATL, ~/src/logs/atl-installed.txt records
# which source tree and commit is installed.
set -uo pipefail
TARGET=${1:?usage: build.sh atl|art|bionic|opensles|harness}
SRC=~/src
LOGS=$SRC/logs
mkdir -p "$LOGS"
HERE=$(cd "$(dirname "$0")" && pwd)
MAKE_JOBS=${MAKE_JOBS:-$(nproc)}
INSTALL=${INSTALL:-1}
CLEAN=${CLEAN:-0}
ATL_SRC=${ATL_SRC:-$SRC/android_translation_layer}
export PKG_CONFIG_PATH=/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}
# shellcheck disable=SC1090
[ -f ~/.cargo/env ] && . ~/.cargo/env

STAMP=$(date +%Y%m%d-%H%M%S)
LOG=$LOGS/build-$TARGET-$STAMP.log

meson_build() { # dir [setup args...]
  local dir=$1; shift
  cd "$dir" || return 1
  if [ "$CLEAN" = 1 ]; then rm -rf builddir; fi
  if [ ! -f builddir/build.ninja ]; then
    # Compiler launchers are baked in at first `meson setup` (meson refuses
    # changing the compiler on reconfigure), so wire ccache there. ccache
    # keys on preprocessed source + compiler signature; objects are
    # bit-identical. Existing builddirs pick it up on their next CLEAN
    # build. NOCACHE=1 skips it.
    if command -v ccache >/dev/null 2>&1 && [ -z "${NOCACHE:-}" ]; then
      CCACHE_DIR=${CCACHE_DIR:-$HOME/.ccache} CCACHE_MAXSIZE=${CCACHE_MAXSIZE:-10G} \
        CC="${CC:-ccache gcc}" CXX="${CXX:-ccache g++}" \
        meson setup builddir --prefix=/usr/local "$@" || return 1
    else
      meson setup builddir --prefix=/usr/local "$@" || return 1
    fi
  fi
  meson compile -C builddir -j "$MAKE_JOBS" || return 1
  if [ "$INSTALL" = 1 ]; then
    sudo meson install -C builddir --quiet || return 1
    sudo ldconfig
  fi
}

record_atl() {
  if [ -n "${ATL_LOCAL_REV:-}" ]; then  # synced from a workstation tree (no .git here)
    printf '%s %s %s\n' "$(date -u +%FT%TZ)" "$ATL_LOCAL_REV" "$ATL_SRC" > "$LOGS/atl-installed.txt"
    return
  fi
  local rev dirty=""
  rev=$(git -C "$ATL_SRC" rev-parse --short HEAD 2>/dev/null || echo "no-git")
  if git -C "$ATL_SRC" rev-parse >/dev/null 2>&1 && [ -n "$(git -C "$ATL_SRC" status --porcelain --untracked-files=no 2>/dev/null)" ]; then
    dirty="+dirty"
  fi
  printf '%s %s%s %s\n' "$(date -u +%FT%TZ)" "$rev" "$dirty" "$ATL_SRC" > "$LOGS/atl-installed.txt"
}

build() {
  case "$TARGET" in
    atl)      meson_build "$ATL_SRC" -Ddownload_tool_deps=true && { [ "$INSTALL" = 1 ] && record_atl; true; } ;;
    bionic)   meson_build "$SRC/bionic_translation" ;;
    opensles) meson_build "$SRC/libopensles-standalone" ;;
    art)
      cd "$SRC/art_standalone" || return 1
      if [ "$CLEAN" = 1 ]; then make clean; fi
      make -j"$MAKE_JOBS" ____LIBDIR=lib64 || return 1
      if [ "$INSTALL" = 1 ]; then sudo make ____LIBDIR=lib64 install && sudo ldconfig; fi ;;
    harness)
      cd "${HARNESS_DIR:?}" || return 1
      cargo build --release ;;
    *) echo "unknown target $TARGET (atl|art|bionic|opensles|harness)" >&2; return 2 ;;
  esac
}

start=$(date +%s)
echo "== build $TARGET on $(uname -m), log $LOG"
build > "$LOG" 2>&1
rc=$?
secs=$(( $(date +%s) - start ))
ln -sf "$LOG" "$LOGS/build-$TARGET-latest.log"
python3 "$HERE/builderrors.py" "$LOG" --json > "$LOGS/build-$TARGET-latest.json" 2>/dev/null || true

if [ "$rc" = 0 ]; then
  warns=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["warnings"])' \
    "$LOGS/build-$TARGET-latest.json" 2>/dev/null || echo "?")
  echo "BUILD OK $TARGET in ${secs}s ($warns warnings)"
  [ "$TARGET" = atl ] && [ "$INSTALL" = 1 ] && echo "installed: $(cat "$LOGS/atl-installed.txt")"
else
  echo "BUILD FAILED $TARGET (exit $rc) after ${secs}s"
  python3 "$HERE/builderrors.py" "$LOG" --max 12 || true
  echo "-- last lines of $LOG:"
  tail -n 12 "$LOG"
fi
exit "$rc"
