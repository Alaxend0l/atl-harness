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
`interactions/org.fossify.musicplayer.txt` (library + playback; the last step
fails until playback works).

Good candidates from triage.md ("claude" items with a visible symptom):
AlertDialog.Builder.setView (custom view between message and buttons),
gradient shaders (`<shape><gradient>` draws black), TextView maxLines/ellipsize,
BitmapShader/rounded images (Music/Gallery covers draw as flat pink squares).
