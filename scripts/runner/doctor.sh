#!/usr/bin/env bash
# Runner health checks. Sent inline by `rig doctor` (works before the first sync).
# Env: EXPECT_ARCH, HARNESS_DIR (absolute).
# Output: one line per check, "ok|warn|fail<TAB>check<TAB>detail<TAB>hint".
set -uo pipefail
# shellcheck disable=SC1090
[ -f ~/.cargo/env ] && . ~/.cargo/env
export PKG_CONFIG_PATH=/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}

r() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}"; }
have() { command -v "$1" >/dev/null 2>&1; }
BOOT="rig bootstrap"

arch=$(uname -m)
if [ "$arch" = "$EXPECT_ARCH" ]; then r ok arch "$arch"
else r fail arch "$arch (runners.toml says $EXPECT_ARCH)" "fix arch in runners.toml"; fi

ps=$(getconf PAGESIZE)
if [ "$ps" = 4096 ]; then r ok page-size "$ps"
else r warn page-size "$ps" "non-4K kernel: native libs and ART AOT may break (README of ATL)"; fi

if [ -f /etc/atl-rootfs-release ]; then r ok distro "rig rootfs: $(cut -d' ' -f2-4 /etc/atl-rootfs-release)"
elif have dnf; then r ok distro "$(. /etc/os-release; echo "$PRETTY_NAME")"
else r warn distro "$(. /etc/os-release; echo "$PRETTY_NAME")" "bootstrap only supports Fedora"; fi

for t in cargo Xvfb import convert android-translation-layer; do
  if have "$t"; then r ok "$t" "$(command -v "$t")"
  else r fail "$t" missing "$BOOT"; fi
done

if v=$(pkg-config --modversion art-standalone 2>/dev/null); then r ok art-standalone "$v"
else r fail art-standalone missing "$BOOT --only art"; fi

A=~/src/android_translation_layer
if [ -d $A/.git ]; then
  r ok atl-src "$(git -C $A log -1 --format='%h %s' 2>/dev/null | cut -c1-60)"
  o=$(git -C $A remote get-url origin 2>/dev/null)
  case "$o" in
    *Alaxend0l/android-translation-layer*) r ok atl-origin "$o" ;;
    *) r warn atl-origin "${o:-none} (not the fork)" "see docs/rig.md: point origin at the fork" ;;
  esac
  pu=$(git -C $A remote get-url --push upstream 2>/dev/null || true)
  case "$pu" in
    DISABLED*|"") r ok upstream-push "${pu:-no upstream remote}" ;;
    *) r fail upstream-push "$pu" "git -C $A remote set-url --push upstream DISABLED-hard-fork-never-push-upstream" ;;
  esac
else r warn atl-src "no checkout at $A" "$BOOT --only sources"; fi
if [ -f ~/src/logs/atl-installed.txt ]; then r ok atl-installed "$(cut -d' ' -f2- ~/src/logs/atl-installed.txt)"
else r warn atl-installed "unknown (built before rig build)" "rig build"; fi

free_gb=$(( $(df -Pk ~ | awk 'NR==2{print $4}') / 1048576 ))
if [ "$free_gb" -ge 20 ]; then r ok disk "${free_gb} GB free"
else r warn disk "${free_gb} GB free" "corpus + ART build want 20+ GB"; fi

mem_gb=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
r ok memory "${mem_gb} GB available, $(nproc) cpus"

if [ -d "$HARNESS_DIR" ]; then
  h=$(cd "$HARNESS_DIR" && { find src -type f; ls Cargo.toml Cargo.lock config*.toml 2>/dev/null; } \
      | LC_ALL=C sort | xargs -d '\n' sha256sum | sha256sum | cut -d' ' -f1)
  r ok repo-hash "$h"
  if [ -x "$HARNESS_DIR/target/release/atlh" ]; then r ok atlh-built "target/release/atlh"
  else r warn atlh-built "not built" "rig atlh --help (builds on first use)"; fi
else
  r fail repo-hash "no checkout at $HARNESS_DIR" "rig sync"
fi

echo "__clock__ $(date +%s.%N)"
