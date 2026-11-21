<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Agent tasks: what to work on, in order

For opencode (interactive, with the owner) and the overnight loop. Read
`AGENTS.md` first ("Several agents at once": own worktree, claim the runner
in HANDOFF.md, gate before merging), then the task's brief. Every brief has:
symptom, current code with line numbers, what Android does, small numbered
steps, a failing check to write **first** (`test-first.md`), and "done when".

Briefs were researched by Claude against fork main `9a2cd6c4` (2026-11-21);
line numbers drift as code changes: re-read the code before editing.

## Order (most visible to the owner first)
| # | Task | Brief | Who | Runner | Notes |
|---|---|---|---|---|---|
| 1 | **Phone audio** (Music is silent) | `audiotrack.md` ("Also found on the phone") | opencode | phone (owner listens) | first confirm the rootfs has `pipewire-alsa` (`ls ~/atl-root/usr/lib64/alsa-lib`); then the open-failure segfault, then the blocking/position bugs |
| 1b | **Music "erratic, sometimes restarting"** (owner, phone) | below | opencode | phone | evidence: one session's log has `onDestroy` 7 times (activity recreated again and again: check what makes ATL destroy/recreate it: configuration changes on window resize? `android:configChanges` handling in ATL's Activity), plus `NullPointerException ... Drawable.mutate()` on a null drawable. Log: `~/.local/state/atl/org.fossify.musicplayer.log` |
| 1c | **Gramophone** start chain (installed on the phone) | `atlh missing-apis` | opencode | mini | fork branch `fix/media-playlists` fixed Playlists, Typeface, RippleDrawable radius; next: `StorageVolume.getUuid`, androidx `WeightTypeface` reflection (`nativeCreateFromTypeface`) |
| 1d | **Flow** (installed on the phone) | below | opencode | mini | widget stage 1 is done (fork main 63bf04e7): Flow starts and stays alive but renders **blank** (Jetpack Compose: find what Compose's AndroidComposeView needs from ATL: RenderNode/Canvas/HardwareRenderer paths); then `AudioAttributes.getVolumeControlStream`. Test with `rig -r mini smoke atl-harness/work/apks/io.github.aedev.flow_305.apk` (not in the F-Droid corpus) |
| 2 | Crashes from `AnimationUtils.loadInterpolator` (File Manager, Music progress bar) | `interpolators.md` | opencode | mini | Group A (Java only) first |
| 3 | Text 1.33x too big everywhere; Paint measurement | `text-and-fonts.md` steps 1-5 | opencode | mini + screenshots | step 1 changes every app's look: compare screenshots before/after |
| 4 | Pink album covers (Glide fails silently) | HANDOFF item 12 | opencode | mini | find why `content://media/external/audio/albumart/<id>` fails in Glide |
| 5 | Equalizer (toast "An unknown error occurred" in Music) | `audiotrack.md` (Equalizer) | opencode, then loop | any | API first, filtering second |
| 6 | Dialog cancel/back/Escape semantics | `dialogs.md` parts A-B | opencode | mini | part C (native AlertDialog slots) after |
| 7 | Canvas layers, xfermodes, color filters, BitmapFactory sampling | `canvas-layers-and-bitmaps.md` | opencode | mini | read fork `doc/Shaders.md` first |
| 8 | App widgets stage 2 (RemoteViews) | `app-widgets.md` | opencode | mini | stage 1 done 2026-11-21; stage 3 needs the service (designed) |
| 9 | Sandbox: full A/B gate, then `sandbox = true` in configs | `docs/sandbox.md` | opencode | bazzite | see HANDOFF |
| 10 | Typeface, font files, maxLines/ellipsize | `text-and-fonts.md` steps 6-9 | opencode (care) | mini | steps 10-11: ask Claude |
| - | Small fixes with exact hints | `local-queue.json` (`triage.md`) | overnight loop | bazzite | `scripts/agent-loop --hours 8` when bazzite is free |

## Ask Claude (when available) for
- Reviewing merges to the fork's `main` (`git log -p` since the last review).
- Widgets stage 3 implementation review (security: `app-widgets.md`).
- Text steps 10-11; anything native that a brief marks "needs care" and
  that failed after a real attempt (bring the notes).
- Telephony/SMS design (waits on the owner's choice of backend).

## Done tasks (briefs kept for the record)
- `music-playback.md`: resolved (uid identity for media3 trust).
