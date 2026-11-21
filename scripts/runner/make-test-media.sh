#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# The harness's fixed media library ([atl] media_dir): two small albums (Opus
# with a folder cover, FLAC with embedded covers and disc numbers), macOS junk
# files that must be ignored, and a photo. Deterministic, a few seconds, needs
# ffmpeg. Usage: make-test-media.sh [DIR]   (default: work/test-media)
set -euo pipefail
T=${1:-work/test-media}
command -v ffmpeg >/dev/null || { echo "make-test-media: ffmpeg not installed" >&2; exit 1; }
rm -rf "$T.tmp"
A="$T.tmp/Music/Alpha Band - First Album"
B="$T.tmp/Music/Beta - Second (FLAC)"
mkdir -p "$A" "$B" "$T.tmp/Pictures"
ff() { ffmpeg -nostdin -loglevel error -y "$@"; }
for i in 1 2 3; do
  ff -f lavfi -i "sine=frequency=$((300 + i * 100)):duration=$((20 + i))" -c:a libopus \
     -metadata title="Alpha Song $i" -metadata artist="Alpha Band" -metadata album="First Album" \
     -metadata date=2016 -metadata track="$i/3" -metadata genre=Rock "$A/0$i Alpha Song $i.opus"
done
ff -f lavfi -i "color=c=red:s=300x300" -frames:v 1 "$A/cover.jpg"
cp "$A/cover.jpg" "$A/._cover.jpg"   # AppleDouble junk: must not be indexed or used as art
: > "$A/.DS_Store"
ff -f lavfi -i "color=c=blue:s=200x200" -frames:v 1 "$T.tmp/bcover.png"
for i in 1 2; do
  ff -f lavfi -i "sine=frequency=$((500 + i * 50)):duration=15" -i "$T.tmp/bcover.png" \
     -map 0 -map 1 -c:a flac -c:v png -disposition:v attached_pic \
     -metadata title="Beta Tune $i" -metadata artist="Beta" -metadata album_artist="Beta" \
     -metadata album="Second" -metadata date=2020-05-01 -metadata track=$i -metadata disc=2 \
     -metadata genre=Electronic "$B/Beta Tune $i.flac"
done
rm "$T.tmp/bcover.png"
ff -f lavfi -i "color=c=green:s=640x480" -frames:v 1 "$T.tmp/Pictures/photo.png"
# fixed mtimes: the library looks the same on every runner and every run
find "$T.tmp" -exec touch -h -d 2026-01-01T00:00:00Z {} +
rm -rf "$T"; mv "$T.tmp" "$T"
echo "test media: $(find "$T" -type f -not -name '.*' | wc -l) files in $T"
