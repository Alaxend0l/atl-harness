#!/usr/bin/env bash
# DEPRECATED: use scripts/rig (docs/rig.md). Kept for existing shell setups.
# Drive the harness on the remote Linux box from your laptop.
#   export ATLH_REMOTE=user@box           (any ssh destination / ~/.ssh/config alias)
#   export ATLH_REMOTE_BOX=atl            (optional: run inside this distrobox)
#   scripts/remote.sh sync                 push this repo (not work/) to the box
#   scripts/remote.sh atlh rank <run-id>   run any atlh command on the box
#   scripts/remote.sh pull                 copy db + logs/screenshots back (no APKs)
#   scripts/remote.sh shell                interactive shell in the remote dir
set -euo pipefail
: "${ATLH_REMOTE:?set ATLH_REMOTE=user@host}"
DIR="${ATLH_REMOTE_DIR:-atl-harness}"
BOX="${ATLH_REMOTE_BOX:-}"
HERE="$(cd "$(dirname "$0")/.." && pwd)"

# Run a shell command string on the box, inside the distrobox if one is set.
# rustup's env file is sourced explicitly: non-interactive shells skip it.
remote() {
  local cmd=". ~/.cargo/env 2>/dev/null; cd $DIR && $1"
  if [[ -n "$BOX" ]]; then
    cmd="distrobox enter $(printf '%q' "$BOX") -- bash -lc $(printf '%q' "$cmd")"
  fi
  ssh -t "$ATLH_REMOTE" "$cmd"
}

case "${1:-}" in
  sync)
    # No -t: changed files get the box's mtime, so cargo rebuilds even if the
    # workstation clock is behind the box's. --checksum skips unchanged files.
    rsync -rlpz --checksum --delete --exclude work/ --exclude target/ --exclude .git/ \
      "$HERE/" "$ATLH_REMOTE:$DIR/" ;;
  atlh)
    shift
    # long runs: use tmux on the box so a dropped ssh doesn't kill them
    remote "cargo run --release --quiet -- $(printf '%q ' "$@")" ;;
  pull)
    mkdir -p "$HERE/work"
    rsync -az --exclude apks/ --exclude index-v2.json \
      "$ATLH_REMOTE:$DIR/work/" "$HERE/work/" ;;
  shell)
    remote "exec bash -l" ;;
  *)
    sed -n '2,9p' "$0"; exit 1 ;;
esac
