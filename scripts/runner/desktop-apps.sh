#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Put Android apps on a runner's desktop/phone home screen, running under ATL.
# Usage (normally via `rig desktop-apps`): desktop-apps.sh install|remove PKG...
# Run from the harness checkout (APKs come from work/apks, names from
# work/index-v2.json). Everything goes under $HOME:
#   ~/.local/bin/atl-app                      launcher wrapper
#   ~/atl-apps/<pkg>.apk                      stable copy of each APK
#   ~/atl-apps/<pkg>.policy.json              its sandbox policy (edit to grant/revoke)
#   ~/.local/bin/atl-sandbox                  the per-app sandbox (docs/sandbox.md)
#   ~/.local/share/applications/atl-<pkg>.desktop
#   ~/.local/share/icons/atl/<pkg>.<png|webp>
#   ~/.local/state/atl/<pkg>.log              each launch's ATL output (for bug reports)
# Apps keep their data in ATL's default data dir between launches.
set -euo pipefail
ACTION=${1:?usage: desktop-apps.sh install|remove PKG...}
shift
BIN=~/.local/bin
APPS=~/atl-apps
DESK=~/.local/share/applications
ICONS=~/.local/share/icons/atl
mkdir -p "$BIN" "$APPS" "$DESK" "$ICONS" ~/.local/state/atl

write_wrapper() {
  # the per-app sandbox (docs/sandbox.md); runs on the host, outside the rootfs
  cp "$(dirname "$0")/atl-sandbox" "$BIN/atl-sandbox"
  chmod 755 "$BIN/atl-sandbox"
  cat > "$BIN/atl-app" <<'EOF'
#!/bin/sh
# atl-app APK [ATL ARGS...]: run an Android app with ATL in its per-app sandbox
# (~/.local/bin/atl-sandbox; policy: <APK minus .apk>.policy.json, editable).
# With ~/atl-root (rig bwrap rootfs) ATL comes from the rootfs, read-only.
# ATL_NO_SANDBOX=1 atl-app ...: the old unsandboxed launch, for comparison.
apk=$1; shift
name=$(basename "$apk" .apk)
log="$HOME/.local/state/atl/$name.log"
mkdir -p "$HOME/.local/state/atl"
echo "=== $(date -Is) atl-app $apk $*" >> "$log"
root=""
[ -d "$HOME/atl-root" ] && root="--rootfs $HOME/atl-root"
if [ -n "$ATL_NO_SANDBOX" ]; then
  echo "(ATL_NO_SANDBOX: unsandboxed)" >> "$log"
  if [ -d "$HOME/atl-root" ]; then
    rt=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
    exec bwrap --bind "$HOME/atl-root" / --bind "$HOME" "$HOME" --bind /tmp /tmp \
      --proc /proc --dev /dev --dev-bind-try /dev/dri /dev/dri --dev-bind-try /dev/snd /dev/snd \
      --tmpfs /run --bind "$rt" "$rt" \
      --ro-bind /etc/resolv.conf /etc/resolv.conf --ro-bind /etc/passwd /etc/passwd \
      --ro-bind /etc/group /etc/group --ro-bind-try /etc/machine-id /etc/machine-id \
      --ro-bind-try /etc/localtime /etc/localtime \
      --setenv PATH /usr/local/bin:/usr/bin:/bin --die-with-parent \
      android-translation-layer "$apk" "$@" >> "$log" 2>&1
  fi
  exec android-translation-layer "$apk" "$@" >> "$log" 2>&1
fi
# shellcheck disable=SC2086
exec python3 "$HOME/.local/bin/atl-sandbox" $root "$apk" "$@" >> "$log" 2>&1
EOF
  chmod 755 "$BIN/atl-app"
}

install_one() {
  local pkg=$1 apk
  apk=$(ls work/apks/"$pkg"_*.apk 2>/dev/null | sort -V | tail -n 1)
  [ -n "$apk" ] || { echo "skip $pkg: not in work/apks (rig atlh fetch --pkg $pkg)"; return 0; }
  cp "$apk" "$APPS/$pkg.apk"
  # sandbox policy from the manifest's permissions; never overwrite an edited one
  [ -f "$APPS/$pkg.policy.json" ] || python3 "$(dirname "$0")/atl-sandbox" --policy-for "$APPS/$pkg.apk" > "$APPS/$pkg.policy.json"
  python3 -I - "$pkg" "$APPS/$pkg.apk" "$ICONS" "$DESK" "$BIN/atl-app" <<'EOF'
import hashlib, json, os, sys, urllib.request, zipfile
pkg, apk, icons, desk, wrapper = sys.argv[1:]
name, meta = pkg, {}
try:
    meta = json.load(open("work/index-v2.json"))["packages"][pkg]["metadata"]
    n = meta.get("name", {})
    name = (n.get("en-US") or next(iter(n.values()), pkg)) if isinstance(n, dict) else n
except Exception:
    pass
icon = "application-x-executable"
with zipfile.ZipFile(apk) as z:
    cands = [i for i in z.infolist()
             if "ic_launcher" in i.filename and i.filename.endswith((".png", ".webp"))
             and not any(x in i.filename for x in ("background", "foreground", "monochrome"))]
    if cands:
        best = max(cands, key=lambda i: i.file_size)
        ext = os.path.splitext(best.filename)[1]
        icon = os.path.join(icons, pkg + ext)
        with open(icon, "wb") as f:
            f.write(z.read(best))
# Release builds often shrink resource names (no ic_launcher.png to find):
# fall back to the icon F-Droid publishes, checked against the index's sha256.
if icon == "application-x-executable":
    ic = meta.get("icon", {})
    ic = ic.get("en-US") or next(iter(ic.values()), None) if isinstance(ic, dict) else None
    if ic and ic.get("name"):
        try:
            data = urllib.request.urlopen("https://f-droid.org/repo" + ic["name"], timeout=30).read()
            if not ic.get("sha256") or hashlib.sha256(data).hexdigest() == ic["sha256"]:
                icon = os.path.join(icons, pkg + os.path.splitext(ic["name"])[1])
                with open(icon, "wb") as f:
                    f.write(data)
        except Exception as e:
            print(f"  (no icon for {pkg}: {e})")
with open(os.path.join(desk, f"atl-{pkg}.desktop"), "w") as f:
    f.write(f"""[Desktop Entry]
Type=Application
Name={name} (ATL)
Comment={pkg} running under Android Translation Layer (fork)
Exec={wrapper} {apk}
Icon={icon}
Terminal=false
Categories=Utility;
X-ATL-Package={pkg}
""")
print(f"installed {pkg}: {name} (icon: {os.path.basename(icon)})")
EOF
}

case "$ACTION" in
  install)
    write_wrapper
    for p in "$@"; do install_one "$p"; done ;;
  remove)
    for p in "$@"; do
      rm -f "$DESK/atl-$p.desktop" "$APPS/$p.apk" "$ICONS/$p".*
      echo "removed $p (app data kept in ~/.local/share/android_translation_layer)"
    done ;;
  *) echo "unknown action $ACTION" >&2; exit 2 ;;
esac
command -v update-desktop-database >/dev/null && update-desktop-database "$DESK" 2>/dev/null || true
