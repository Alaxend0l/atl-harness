#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Debug an app under ATL with jdb on a runner (run via: rig -r mini sh 'scripts/runner/jdb-app.sh ...').
# Starts the app on a private Xvfb with JDWP (ATL's JDWP_LISTEN), lets it start,
# sets a breakpoint, performs taps, then runs jdb commands at the breakpoint.
#
#   jdb-app.sh PKG "BREAKPOINT" "TAPS" "JDB COMMANDS"
#   PKG          package; the APK is work/apks/PKG_*.apk
#   BREAKPOINT   jdb spec, e.g. "android.media.AudioTrack.play" or "SomeClass:123"
#                (classes of the app are obfuscated: take names from a stack trace in its log)
#   TAPS         space-separated x,y screen taps after startup (1080x1920), e.g. "968,1880 300,150"
#   COMMANDS     ';'-separated jdb commands at the breakpoint, e.g. "where;locals;dump this"
# Env: SETTLE (s before the breakpoint, default 30), MEDIA (ATL_MEDIA_DIRS, default work/test-media).
# Output: /tmp/jdb-out.txt (jdb) and /tmp/jdb-app.log (app). Everything has a time limit.
set -u
PKG=$1; BP=$2; TAPS=${3:-}; CMDS=${4:-where}
cd "$(dirname "$0")/../.."
apk=$(ls "$PWD"/work/apks/"$PKG"_*.apk | head -1)
MEDIA=${MEDIA:-$PWD/work/test-media}
H=$(mktemp -d); mkdir -p "$H/.local/share"
Xvfb :78 -screen 0 1080x1920x24 >/dev/null 2>&1 & X=$!
sleep 1
( cd "$H"; HOME=$H XDG_DATA_HOME=$H/.local/share XDG_CACHE_HOME=$H/.cache XDG_CONFIG_HOME=$H/.config \
  DISPLAY=:78 GDK_BACKEND=x11 GDK_DISABLE=glx ATL_MEDIA_DIRS=$MEDIA JDWP_LISTEN=8711 \
  timeout 150 android-translation-layer "$apk" -w 1080 -h 1920 > "$H/log" 2>&1 & )
sleep 3
F=$H/jdbin; mkfifo "$F"
( timeout 140 jdb -attach localhost:8711 < "$F" > /tmp/jdb-out.txt 2>&1 ) &
exec 3>"$F"
echo "cont" >&3
sleep "${SETTLE:-30}"
echo "stop in $BP" >&3
sleep 1
for t in $TAPS; do DISPLAY=:78 xdotool mousemove "${t%,*}" "${t#*,}" click 1; sleep 4; done
sleep 4
IFS=';' read -ra C <<< "$CMDS"
for c in "${C[@]}"; do echo "$c" >&3; sleep 1.5; done
echo "clear $BP" >&3; echo "cont" >&3; sleep 1; echo "exit" >&3
exec 3>&-
sleep 2
pkill -f "Xvfb :78"; kill "$X" 2>/dev/null
cp "$H/log" /tmp/jdb-app.log
grep -v '^>\s*$' /tmp/jdb-out.txt
