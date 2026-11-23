# Element-addressed interaction scripts (planning brief)

Status: **P0 done 2026-11-23** (media keys, fork `dc58bf2a` on
`fix/fixcov-crashes`); **P1 done 2026-11-23, witnessed and merged**
(fork dumper + harness `find`/`tap $el` in Rust, owner instruction;
fork merge `56206f55` on `local-fixes`); P2–P4 planned (owner proposal
2026-11-23, see roadmap "Harness: element-addressed interaction scripts").

## P1 (done 2026-11-23)

- Fork (branch `feat/element-dump`, merged to `local-fixes` as `56206f55`;
  dumper `3694d263` + regenerated JNI headers `0b1c3695`):
  `src/api-impl/android/atl/ATLElementDump.java` — daemon thread that,
  **only when `ATL_ELEMENT_DUMP=DIR` is set** (unset ⇒ no thread, no
  behavior change, gates stay comparable), posts a view-tree walk to the
  main looper (real: `api-impl-jni/android_os_MessageQueue.c` pumps
  `Looper.loop()` from GLib) every ~1 s (400 ms after a `<DIR>/request`
  touch) and writes `<DIR>/views.json` atomically: `{v:1,nodes:[{c,l,t,
  r,b,en?,clk?,txt?,desc?}]}`, bounds window-local px. Hooked from the
  `Window` constructor (covers Activities and Dialogs). The harness
  points `ATL_ELEMENT_DUMP` under the app's `XDG_DATA_HOME` — the one
  dir `atl-sandbox` re-binds rw — so sandboxed runs work without new
  binds. Merge gate: `fc75` (bazzite, corpora/fixcov.txt, build
  `local:0b1c3695`): **0 improved / 0 regressed** vs `fc73`.
- Harness (`src/interact.rs` + `src/runner.rs`): new steps `find "NAME"
  [class=X] [nth=N] [timeout S]`, `wait-find …`, `tap $el`,
  `long-press $el [SECS]`. The runner loads the script **before**
  spawning and sets `ATL_ELEMENT_DUMP` under the app data dir only when
  the script uses these steps; `$el` centres are mapped to root-window
  absolute pixels via `xdotool getwindowgeometry` of the ATL window
  (never the blank-scoring crop). Matching: text/content-desc,
  case-insensitive, disabled widgets excluded, whitespace-squashed; an
  **exact match beats substring hits** (`find "Tracks"` → the tab, not a
  "9 Tracks" subheader); nodes whose centre is off the Xvfb screen never
  match (kills ViewPager neighbour pages that live at x>1080 or x<0 in
  the dump). The matched tree is kept as `<run dir>/elements.json` and
  the detail line shows window origin, e.g. `$el = "Tracks" (kk2) at
  (968,1899) 40x31, match 1 of 6, win (0,0)`. A failed find fails the
  step **and lists the app's exposed elements** in the run record.
  Parser, matcher and geometry unit-tested (25 tests).
- Witness (P1 exit criterion) **passed** on oneplus12, run `p1w10`,
  `interact_ok=1`, app `org.fossify.musicplayer` (v18; its bottom tabs
  are Playlists / Folders / Artists / Albums / Tracks; media library is
  the phone's `~/Music` — King Gizzard "Nonagon Infinity" — so this
  script lives in `work/interactions-p1/`, not in tracked
  `interactions/`, which assumes the `make-test-media.sh` library):
  `find`+`tap $el` on the **Tracks** tab (2.45 % pixels changed), then
  **Albums** (2.67 %), then **Tracks** again (2.72 %) — every step finds
  the element fresh after the previous one changed the whole layout;
  then `find "Big Fig Wasp"` + `tap $el` starts playback **and**
  navigates to the Now Playing screen (3.67 % changed) and
  `expect-text "Big Fig Wasp"` passes. Gramophone itself was *not*
  witnessable this way: its UI is Compose (one opaque
  `AndroidComposeView` node), which needs the P3 accessibility pass.
  The script (`work/interactions-p1/org.fossify.musicplayer.txt`,
  re-create with scp to the runner if lost — `rig sync` skips `work/`):

  ```
  shot boot
  wait-find "Tracks" timeout=40
  tap $el
  shot tracks_tab
  expect-changed
  wait-find "Albums" timeout=10
  tap $el
  shot albums_tab
  expect-changed
  wait-find "Tracks" timeout=10
  tap $el
  shot tracks_again
  expect-changed
  wait-find "Big Fig Wasp" timeout=10
  tap $el
  wait 1.5
  shot now_playing
  expect-changed
  expect-text "Big Fig Wasp"
  ```
- Follow-ups seen during the witness:
  - Tapping a Fossify track-row **subline** TextView (album • count)
    lands but does nothing in the app, while the **title** TextView
    works; both rows are handled by the parent. Worth a triage brief
    (click propagation from a child without a listener to its parent —
    the fork's `on_click`/`performClick` in `WrapperWidget.c` calls
    `callOnClick()` on the hit view only).
  - The dumper exposes obfuscated label classes (`kk2` = Fossify's tab
    TextView); `class=` filters should document that classes may be
    obfuscated in release APKs.
  - P2: convert the tracked `interactions/*.txt` scripts to
    element-addressed steps where the app uses classic views.

## Why
Fixed-coordinate witness scripts lie silently: when the app layout or
state shifts, taps hit the wrong widget and the run records a *fake*
failure. Witnessed 2026-11-23: the Gramophone pause/resume witness's
(64,336) taps, after the first cycle, opened a Details screen of another
song; the resulting "zero AudioTrack calls" wedge was an artifact and
cost a session to see through.

## Facts established (2026-11-23, from the `atl-fixcov` tree @ `f52ecb55`)
- Harness (`src/interact.rs`): steps `wait/tap/long-press/type/key/shot/
  expect-changed/expect-text`; taps/keys injected with xdotool on the
  runner DISPLAY; `shot` = `import` of the **root window** (absolute
  coords — same coordinate space as taps); `expect-text` shells
  tesseract for plain text (no word boxes yet).
- Fork view layer is real Java: `View` has `getLeft/Top/Right/Bottom`,
  `contentDescription`; `ViewGroup.getChildAt/getChildCount`;
  `TextView.getText`. Roots exist in `ViewRootImpl.mView`, but there is
  **no global window registry** (`WindowManagerGlobal` is a stub) — a
  dump needs one (e.g. a static weak-ref list appended in the
  `ViewRootImpl` constructor).
- Every Android view is mirrored by a GTK `WrapperWidget`
  (`src/api-impl-jni/widgets/WrapperWidget.c`) that dispatches
  MotionEvents incl. scroll — hit-testing and scroll injection already
  exist in the fork.
- `ATL_DUMP_NODES` dumps **GSK render nodes** (graphics), not the view
  tree — not the base for this feature.
- **Compose problem:** Gramophone's APK contains the androidx Compose
  runtime (1725 `androidx/compose` strings in its dex): its UI is one
  opaque `AndroidComposeView`. Compose only maintains its semantics tree
  when accessibility is enabled, and the fork's a11y layer is a stub
  (`AccessibilityNodeInfo.obtain()` returns null) — so a plain view dump
  sees a single node for Gramophone. Fossify-classic apps dump fine.
- No `XF86*` → `KEYCODE_MEDIA_*` key mapping found in the fork (only the
  KeyEvent constants); whether xdotool media keys reach apps is untested.

## Design — finder modules (owner revision 2026-11-23)
Rather than one dumper, the harness resolves `find` through interchangeable
**finder modules**, picked by detected UI framework with an automatic
fallback chain, and every run records which module answered (and what it
saw) for triage.

Key observation keeping the module count small: on Android, frameworks
expose their element trees through the *same* platform API —
`AccessibilityNodeInfo`. Compose (semantics), Flutter (its a11y bridge),
Godot 4.3+, WebView content and AccessKit (egui) all land there. So:

- **M-view** — walk the fork's Java `View` tree (classic apps; also the
  grafting skeleton for all others).
- **M-a11y** — walk `View.createAccessibilityNodeInfo()` + node children
  (bounds-in-screen, text, content-description, actions). One module
  covers Compose/Flutter/Godot-era engines *provided* ATL (a) has a real
  minimal `AccessibilityNodeInfo` object model and (b) reports
  accessibility enabled — both env-gated (`ATL_ELEMENT_DUMP=DIR`), so
  apps that don't ask never change behaviour and gates stay comparable.
- **M-ocr** — tesseract TSV word boxes; universal last resort for
  canvas-drawn UIs with no semantics provider (SDL/libGDX/older engines)
  or as cross-check. Text only: icon-only affordances unaddressable —
  recorded honestly per app.
- **M-engine** (future, only if M-a11y proves insufficient for a specific
  engine): per-engine channel, e.g. FlutterJNI's a11y channel. Deliberately
  deferred: add modules when corpus evidence demands, not in advance.

**Detection**: harness-side APK fingerprint at fetch/corpus time (dex
string/`lib*.so` markers: `androidx/compose`, `libflutter.so`, `libgodot*`,
SDL…), stored per app; drives module *preference order*, not a hard
switch (apps mix frameworks — Gramophone is Compose inside classic Activity
decor). When both view and a11y dumps exist, trees are **merged** (a11y
subtrees grafted under their host views), so `find` sees one unified tree.

### Tier A specifics (M-view)
- `api-impl` Java dumper, gated by env `ATL_ELEMENT_DUMP=DIR`: started
  from ActivityThread startup as a daemon thread; dumps on request file
  (or every ~1.5 s) by walking registered roots on the **main looper**
  (post a Runnable — avoids torn reads of bounds).
- JSON per node: class, `resource-id` name when available, text,
  contentDescription, clickable, enabled, bounds (window-local px).
- `ViewRootImpl` gets the registry. dx constraints: anonymous classes
  only; remember `src/api-impl/meson.build` explicit `srcs` gotcha.

### Harness steps (all modules)
`find "NAME" [class=…] [nth=N]` (and `find-text`), `tap $el`,
`long-press $el`, `wait-find "NAME" [timeout]` (replaces blind sleeps);
element-not-found = hard FAIL + element list into the run log and a
distinct failure signature kind. Raw-coordinate steps stay for edge
cases. `swipe X1 Y1 X2 Y2 [steps] [ms]` + semantic `swipe up|down|
left|right` (window-relative, xdotool button press/move/up).
Semantic key names (`key media-play-pause`) ride the new always-on
XF86→KEYCODE_MEDIA mapping (owner decision 2026-11-23: it is standard
hardware input, not gated).

### Coordinate mapping (bites)
Dump bounds are window-local; screenshots/taps are root-window absolute.
Harness must translate via window origin (`xdotool getwindowgeometry`) —
do not reuse `crop_top` from blank-scoring; it is a scoring crop, not an
origin.

## Phases
- **P0 (fork, small) — done 2026-11-23, oneplus12.** `XF86Audio* →
  KEYCODE_MEDIA_*` keysym mapping + window-level intercept + semantic
  `key media-*` names in the harness. Fork commit `dc58bf2a` on
  `fix/fixcov-crashes`; witness: Gramophone run `grres18`
  (`work/interactions-mp/org.akanework.gramophone.txt`) —
  `key media-pause` → `handled=true` → `calling AudioTrack.pause()`;
  `key media-play` → PLAY_PAUSE expands to PLAY from the stored PAUSED
  state (`origKey=85 sentKey=126 handled=true`) and media3's
  notification action flips Play→Pause. Same witness then
  **reproduces the owner's resume bug deterministically**: after the
  resume no `AudioTrack.play()` follows and media3 logs
  `audioTrackUnderrun` right after pause (see HANDOFF). **Update same
  day: that resume bug is fixed — non-monotonic
  `getPlaybackHeadPosition` under PipeWire, fork `e20d7f16`
  (witnesses grres22..grres24).**
  Implementation notes: media keys are intercepted by a
  **capture-phase** `GtkEventControllerKey` on the main window
  (installed from `main.c`) — the per-view controllers only fire along
  a GTK focus path, and ATL view-based apps have never received key
  events at all (wrappers are focusable only when overriding key
  methods; `Window.take_input_queue` is called only by
  NativeActivity). AOSP's system-server role (session registry,
  PLAY_PAUSE expansion, `Callback.onMediaButtonEvent` default switch)
  is replicated in `MediaSession.java`. GTK ≥ 4.22 note:
  `gtk_event_controller_set_phase` is gone — use
  `gtk_event_controller_set_propagation_phase` + `GTK_PHASE_CAPTURE`.
- **P1 spike** — `ViewRootImpl` registry + gated JSON view dump +
  harness `find`/`tap $el`, demonstrated on a Fossify app: a witness
  that taps "Play" by name and survives a library re-sort. Half-day scale.
- **P2** — `swipe`, `wait-find`, failure-signature kind, `docs/usage.md`
  + script examples; convert the flakiest existing witness scripts.
- **P3 (fork, spike)** — minimal `AccessibilityNodeInfo` model +
  env-gated `AccessibilityManager.isEnabled()=true`; Gramophone go/no-go:
  does Compose populate nodes? Yes → M-a11y unified tree; no → M-ocr
  fallback for Compose until M-engine work is justified by corpus pain.
- **P4** — M-ocr word boxes (tesseract TSV) + framework fingerprinting
  into the corpus (`find` preference order per app, module-of-record
  logged per run).
- Every phase: full-corpus gate 0/0 expected (fork changes env-gated
  except P0, which only adds previously-absent hardware-key handling).

## Risks / open questions
1. **a11y convergence assumptions**: Compose versions differ in what they
   require before populating semantics (action ids, state masks, window
   connection). Handled by the P3 spike + M-ocr fallback, not by promise.
2. **Framework fingerprint false positives** (mixed Compose+View apps,
   bundled-but-unused libraries): the fingerprint drives module
   *preference*; merged trees decide truth; the run log names the module
   that answered.
3. **Bounds→screen mapping** under the runner's WM/decorations: harness
   maps via `xdotool getwindowgeometry`, not the blank-scoring crop.
4. **Dump thread safety**: walk on the main looper (posted Runnable).
5. **a11y-enable perturbation**: enabling AccessibilityManager changes
   Compose app behaviour slightly; strictly env-gated so gates stay
   comparable.
6. **Multi-window**: dialogs/popups → dump must walk all roots, and
   `find` must pick/flag the topmost match.
7. **Tesseract quality** on thin/light fonts and phone-scale DPI —
   M-ocr matching needs fuzz, like `expect-text` already does.
8. Keep all existing coordinate scripts working unchanged (gate
   baselines compare against them).
