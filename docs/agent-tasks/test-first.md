<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Turning a hard fix into a checkable one (test first)

The overnight loop only commits changes that don't regress the gate, and the
gate only sees what runs: app startup plus `interactions/*.txt`. A fix for a
dialog, click or rendering bug passes even if it's wrong, unless a script
checks that behaviour. So for anything beyond a tiny fix:

1. **Find an app that shows the bug** (corpus apps in `corpora/*.txt`; APKs in
   `work/apks/` on the runners). `rig -r mini smoke work/apks/<apk> --settle 20`
   gives a screenshot; `rig -r mini atlh run --pkg <pkg> --force` a log.
2. **Write `interactions/<pkg>.txt`** that drives the app to the bug and
   checks the correct result (syntax: docs/usage.md "Interaction scripts").
   Get coordinates from the screenshot (1080x1920). Prefer `expect-text`
   (OCR) for text; `expect-changed` that something happened.
3. **Run it on the unfixed fork: it must FAIL** (otherwise it doesn't test the
   bug). Run it twice: it must fail the same way (not flaky).
4. Commit the script (harness repo). Now the gate checks the behaviour:
   a fix that doesn't make it pass isn't an improvement, and once it passes,
   breaking it again is a regression (`compare` shows ALIVE+ok -> ALIVE+FAIL).
5. Then fix (yourself, or queue it for the loop in
   `docs/agent-tasks/local-queue.json` with a precise `fix_hint`).

Examples: `interactions/org.fossify.math.txt` (7+8=15),
`interactions/org.fossify.musicplayer.txt` (library + playback).

Good candidates from triage.md ("claude" items with a visible symptom):
AlertDialog.Builder.setView (custom view between message and buttons),
gradient shaders (`<shape><gradient>` draws black), TextView maxLines/ellipsize,
BitmapShader/rounded images (Music/Gallery covers draw as flat pink squares).

## Status (2026-11-21)

- **setView**: done. `interactions/org.fossify.musicplayer.txt` now drives
  Playlists tab -> overflow -> "Create new playlist" -> types "TestList" ->
  `expect-text "TestList"`. On ATL main 7cb4cd2b the dialog renders only its
  title and Cancel/OK; the custom view (title EditText) is dropped
  (mini run nav10 screenshot). Verified failing twice identically (runs
  setv1..setv4: only that step fails, the library/playback steps still pass).
  Caveat: Fossify's builder is the androidx one from the APK's dex, so the
  observed drop may live in the custom-view slot of that layout path rather
  than in ATL's own `android.app.AlertDialog.Builder.setView` — the script
  only pins the visible symptom, which is what the gate needs.
- **maxLines/ellipsize**: not scripted yet. The symptom ("label always wraps
  freely") is not expressible with the current DSL: `expect-text` is
  presence-only, and truncated vs wrapped text shows the same prefix — a
  correct render shows "long title…", the buggy one the same words unwrapped,
  so no OCR presence-check fails for the right reason. Needs a DSL step first
  (`expect-no-text`, or region/line-count OCR) — harness task, add to
  roadmap/queue.
- Note when scripting Fossify Music: its tab bar sits at y=1880 (tabs at
  x=108/324/539/752/968) and the app's navigation/playback state persists
  across runs (the sandbox bind-mounts the ATL data dir); wipe
  `~/.local/share/android_translation_layer/<pkg>_*` on the runner for a
  deterministic launch. Screenshot pixel coordinates are 1:1 with the Xvfb
  screen, but images rendered in chat are scaled — measure coordinates with
  PIL or OCR TSV boxes, not by eye.

