#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Build a self-contained Fedora root filesystem with the ATL stack, for runners
# that can't host a distrobox (e.g. a phone with a full root partition).
# Runs on a bootstrapped runner of the same arch; uses its /usr/local build.
# Usage (normally via `rig rootfs`): make-rootfs.sh OUT_TARBALL
# The tarball unpacks to a tree used with `bwrap` (runners.toml: bwrap_root).
set -euo pipefail
OUT=$1
RELEASE=$(. /etc/os-release; echo "$VERSION_ID")
ROOT=$(mktemp -d /var/tmp/atl-rootfs.XXXXXX)
trap 'sudo rm -rf "$ROOT"' EXIT

# Same package set as bootstrap (build deps included, so rebuild-atl and the
# harness build work inside the rootfs too).
# shellcheck disable=SC2046 # one package per word
sudo dnf install -y -q --installroot="$ROOT" --releasever="$RELEASE" --use-host-config \
  --setopt=install_weak_deps=False bash coreutils findutils which procps-ng sed grep gawk \
  ca-certificates curl tar gzip zstd xz rpm $(grep -v '^#' "$(dirname "$0")/packages.txt")

# The ATL stack built by bootstrap, and the loader/pkg-config setup for it.
# Lima VMs also put their container tooling (nerdctl-full) in /usr/local; skip it.
LIMA_EXCLUDES=(--exclude='./bin/buildctl' --exclude='./bin/buildg' --exclude='./bin/buildkit*'
  --exclude='./bin/bypass4netns*' --exclude='./bin/containerd*' --exclude='./bin/ctd-decoder'
  --exclude='./bin/ctr' --exclude='./bin/ctr-*' --exclude='./bin/fuse-overlayfs' --exclude='./bin/gomodjail'
  --exclude='./bin/lima-guestagent' --exclude='./bin/nerdctl*' --exclude='./bin/rootless*' --exclude='./bin/runc'
  --exclude='./bin/slirp4netns' --exclude='./bin/stargz-*' --exclude='./bin/tini' --exclude='./libexec/cni'
  --exclude='./lib/systemd' --exclude='./share/doc/nerdctl*')
sudo tar -C /usr/local "${LIMA_EXCLUDES[@]}" -cf - . | sudo tar -C "$ROOT/usr/local" -xf -
sudo cp /etc/ld.so.conf.d/local.conf "$ROOT/etc/ld.so.conf.d/"
sudo cp /etc/profile.d/zz-local-pkgconfig.sh "$ROOT/etc/profile.d/"
sudo chroot "$ROOT" ldconfig

# Inside bwrap the runner's user owns the whole tree, so sudo is unnecessary:
# a shim keeps scripts that call it (bootstrap, rebuild-atl) working.
sudo tee "$ROOT/usr/local/bin/sudo" >/dev/null <<'EOF'
#!/bin/sh
# rig bwrap rootfs: the invoking user owns this tree; run the command directly.
while [ $# -gt 0 ]; do case "$1" in -*) shift ;; *) break ;; esac; done
exec "$@"
EOF
sudo chmod 755 "$ROOT/usr/local/bin/sudo"
echo "atl-rootfs fedora-$RELEASE $(uname -m) $(date -u +%FT%TZ) from $(cat /proc/sys/kernel/hostname)" \
  | sudo tee "$ROOT/etc/atl-rootfs-release" >/dev/null

sudo rm -rf "$ROOT/var/cache/dnf" "$ROOT/var/cache/libdnf5" "$ROOT"/dev/* "$ROOT"/proc/* "$ROOT"/sys/*
sudo tar -C "$ROOT" --numeric-owner -cf - . | zstd -T0 -q -o "$OUT" -f
ls -lh "$OUT"
cat "$ROOT/etc/atl-rootfs-release"
