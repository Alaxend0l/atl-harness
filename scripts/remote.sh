#!/usr/bin/env bash
# Drive the harness on the remote Linux box from your laptop.
#   export ATLH_REMOTE=user@box           (any ssh destination / ~/.ssh/config alias)
#   scripts/remote.sh sync                 push this repo (not work/) to the box
#   scripts/remote.sh atlh rank <run-id>   run any atlh command on the box
#   scripts/remote.sh pull                 copy db + logs/screenshots back (no APKs)
#   scripts/remote.sh shell                interactive shell in the remote dir
set -euo pipefail
: "${ATLH_REMOTE:?set ATLH_REMOTE=user@host}"
DIR="${ATLH_REMOTE_DIR:-atl-harness}"
HERE="$(cd "$(dirname "$0")/.." && pwd)"

case "${1:-}" in
  sync)
    rsync -az --delete --exclude work/ --exclude target/ --exclude .git/ \
      "$HERE/" "$ATLH_REMOTE:$DIR/" ;;
  atlh)
    shift
    # long runs: use tmux on the box so a dropped ssh doesn't kill them
    ssh -t "$ATLH_REMOTE" "cd $DIR && cargo run --release --quiet -- $(printf '%q ' "$@")" ;;
  pull)
    mkdir -p "$HERE/work"
    rsync -az --exclude apks/ --exclude index-v2.json \
      "$ATLH_REMOTE:$DIR/work/" "$HERE/work/" ;;
  shell)
    ssh -t "$ATLH_REMOTE" "cd $DIR && exec \$SHELL -l" ;;
  *)
    sed -n '2,8p' "$0"; exit 1 ;;
esac
