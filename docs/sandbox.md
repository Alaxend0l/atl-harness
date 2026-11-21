<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Per-app sandbox (`scripts/runner/atl-sandbox`)

Every Android app runs in its own bubblewrap sandbox, on the phone (launchers
from `rig desktop-apps`) and, with `[atl] sandbox = true` or `atlh run
--sandbox`, in harness runs.

## Threat model
An APK is untrusted code (Java and native libraries) running as your user.
Without a sandbox it can read your SSH keys, browser profiles and every other
app's data, write anywhere in your home, talk to any D-Bus service (secrets,
accessibility, the system bus), reach the LAN, and snoop on other X11 clients.
The sandbox limits each app to what Android would give it.

## What an app gets
| | Access | Why |
|---|---|---|
| Home | an empty tmpfs, plus **its own** ATL data dir (read-write) | its data and Android external storage live there |
| Media | `~/Music`, `~/Pictures`, `~/Videos`, `~/Downloads` (XDG dirs) per manifest permission: `READ_MEDIA_AUDIO/IMAGES/VIDEO` read-only, `READ_EXTERNAL_STORAGE` all read-only, `WRITE_`/`MANAGE_EXTERNAL_STORAGE` all read-write (permissions with `maxSdkVersion` below ATL's SDK 26 are ignored) | Android's storage permissions; ATL's media library indexes exactly these (`ATL_MEDIA_DIRS`) |
| System | the rootfs (`~/atl-root`) or host `/usr` + `/etc`, **read-only**; private `/tmp` | |
| Processes | own PID and UTS namespaces, own IPC namespace under Wayland (X11 needs the shared one for MIT-SHM); no capabilities; new session (no TIOCSTI); dies with its launcher | |
| Display | the Wayland socket only (X11 display in harness runs, `--x11`) | X11 lets any client read other clients' input and screens |
| Audio | PipeWire and Pulse sockets only; ATL's ALSA output reaches PipeWire through `pipewire-alsa` in the rootfs. Never `/dev/snd`: raw `hw:` playback bypasses the OnePlus 12's speaker protection | playback. Capture can't be separated at the socket level: an app could record without `RECORD_AUDIO` |
| GPU | `/dev/dri`, read-only `/sys/devices` + `/sys/dev/char` for device discovery | |
| Camera | `/dev/video*`, `/dev/media*` only with `CAMERA` | |
| Network | per policy (below); default: nothing without `INTERNET`, **internet only** (no LAN) with it | apps shouldn't reach your LAN unless you say so |
| D-Bus | a filtered session bus (`xdg-dbus-proxy`): portals, notifications, owning `org.mpris.MediaPlayer2.*` and **its own package name** (+ `.*`, like a Flatpak app ID: GApplication registration, UnifiedPush); nothing else; no system bus, no accessibility bus | file chooser/OpenURI via portals, notifications, media controls |

## Network
`"network"` in the policy:
| Value | The app can reach |
|---|---|
| `"none"` | nothing (default without `INTERNET`) |
| `"internet"` | the internet only: no LAN, localhost, link-local, multicast, CGNAT/Tailscale (default with `INTERNET`) |
| `["192.168.10.20:8096", "192.168.10.0/24", "100.64.0.0/10", "[fd7a:115c:a1e0::/48]"]` | the internet **plus** these addresses/networks, optionally one port each |
| `"full"` | everything the host can reach (no filtering) |

Examples: Jellyfin on a LAN server `["192.168.10.20:8096"]`; Home Assistant
`["192.168.10.30:8123"]` (add `"224.0.0.251:5353"` for mDNS discovery);
anything over Tailscale `["100.64.0.0/10", "[fd7a:115c:a1e0::/48]"]`.
Hostnames aren't accepted (resolved addresses can change): use IPs.

How: filtered modes give the app its own user + network namespace
(`unshare`), load an nftables output filter there (DNS goes to pasta's
forwarder, the allow-list, then reject private ranges), connect it with
`pasta` (traffic leaves through the host's sockets, so VPN/Tailscale routes
work; no inbound ports), then start the sandbox's bwrap inside as your own
uid without capabilities. Needs `unshare`, `nft`, `pasta` (packages
`util-linux`, `nftables`, `passt`) on the host or in the rootfs; without them
a filtered policy refuses to start rather than falling back to full access.
On the phone, `passt`/`nftables` come with the next `rig -r oneplus12 rootfs
--from mini` (in scripts/runner/packages.txt).

## Policy files
On the phone each app has `~/atl-apps/<pkg>.policy.json`, written from the
manifest at install and never overwritten afterwards. Edit it to grant or
revoke access:
```json
{"network": ["192.168.10.20:8096"], "media": {"music": "ro"}, "camera": false, "audio": true, "dbus_talk": []}
```
`atl-sandbox --policy-for APP.apk` prints what the manifest asks for.
`ATL_NO_SANDBOX=1` (env of `atl-app`) runs one launch without the sandbox,
for comparing behaviour.

## Limits (not done yet)
- Allow-lists take IPs, not hostnames; no inbound connections to apps.
- No seccomp filter (the PID namespace already prevents ptrace of others).
- Microphone capture via PipeWire/Pulse (above).
- Portals see the app as an unsandboxed host app (no app ID).
- Mediated installs (roadmap: installer stores) build on this sandbox.

## Checking it
`python3 -m unittest scripts/tests/test_atl_sandbox.py` checks the rules
(no network without INTERNET, home is empty except the app's data, `~/.ssh`
never visible, rootfs read-only, D-Bus only through the proxy, ...).
`atlh run --sandbox` vs a normal run + `atlh compare` shows what the sandbox
breaks.

## Rollout
A/B on bazzite (x86_64) with the full corpus, ATL main 9a2cd6c4:
`run --run-id plain1 --no-sandbox` vs `run --run-id sbx1 --sandbox` —
`atlh compare plain1 sbx1`: 52 apps in both runs, 0 improved, 0 regressed
(same outcomes and same interaction results). So `[atl] sandbox = true` is
now the default in `config.toml` and `config.aarch64.toml`; `--no-sandbox`
still overrides per run for comparisons.
