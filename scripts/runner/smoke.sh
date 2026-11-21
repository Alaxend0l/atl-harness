#!/usr/bin/env bash
# Run one APK under ATL on a private Xvfb, screenshot it, print a summary.
# Usage (normally via `rig smoke`): smoke.sh OUT_DIR SETTLE_S CROP_TOP APK [ATL ARGS...]
# Writes OUT_DIR/run.log and OUT_DIR/shot.png; prints key=value lines.
# Mirrors the harness runner's environment (src/runner.rs) so results match.
set -euo pipefail
OUT=$1 SETTLE=$2 CROP=$3 APK=$4
shift 4
[ -f "$APK" ] || { echo "no such apk: $APK" >&2; exit 2; }
mkdir -p "$OUT"
rm -f "$OUT/run.log" "$OUT/shot.png"

H=$(mktemp -d /tmp/atlh-smoke.XXXXXX)
mkdir -p "$H/.local/share" "$H/.config" "$H/.cache"
cleanup() {
  [ -n "${AP:-}" ] && kill -- -"$AP" 2>/dev/null || true
  [ -n "${XP:-}" ] && kill "$XP" 2>/dev/null || true
  rm -rf "$H"
}
trap cleanup EXIT

# -displayfd: Xvfb picks a free display number and writes it to that fd.
Xvfb -displayfd 4 -screen 0 1080x1920x24 -nolisten tcp 4>"$H/display" 2>/dev/null &
XP=$!
for _ in $(seq 100); do
  [ -s "$H/display" ] && break
  sleep 0.1
done
[ -s "$H/display" ] || { echo "Xvfb did not start" >&2; exit 1; }
DISP=":$(head -1 "$H/display")"

start=$(date +%s)
setsid env -u WAYLAND_DISPLAY HOME="$H" XDG_DATA_HOME="$H/.local/share" XDG_CONFIG_HOME="$H/.config" \
  XDG_CACHE_HOME="$H/.cache" DISPLAY="$DISP" GDK_BACKEND=x11 GDK_DISABLE=glx \
  android-translation-layer "$APK" -w 1080 -h 1920 "$@" > "$OUT/run.log" 2>&1 < /dev/null &
AP=$!

alive=yes
for _ in $(seq "$SETTLE"); do
  sleep 1
  kill -0 "$AP" 2>/dev/null || { alive=no; break; }
done

stddev=
if [ "$alive" = yes ]; then
  import -display "$DISP" -window root "$OUT/shot.png" 2>/dev/null || true
  if [ -f "$OUT/shot.png" ]; then
    stddev=$(convert "$OUT/shot.png" -chop "0x$CROP" -format '%[fx:standard_deviation]' info: 2>/dev/null || true)
  fi
fi
code=
if [ "$alive" = no ]; then wait "$AP" && code=0 || code=$?; fi

echo "display=$DISP"
echo "alive_after_settle=$alive"
echo "exit_code=$code"
echo "seconds=$(( $(date +%s) - start ))"
echo "screen_stddev=$stddev"
# ATL logs each intent filter it scans and stops at the one it launches: the
# last one logged is the chosen activity (or activity-alias).
echo "activity=$(grep -ao 'ActivityIntentInfo{[^}]*}' "$OUT/run.log" | tail -n 1 || true)"
echo "reached_onResume=$(grep -aq -- '- onResume -' "$OUT/run.log" && echo yes || echo no)"
echo "exceptions=$(grep -acE '^[[:space:]]*(Caused by: )?[a-zA-Z_$.]+(Exception|Error)\b' "$OUT/run.log" || true)"
