# Handoff: state of atl-harness (2026-11-21, end of day 1)

Read this first, then `AGENTS.md`, `docs/roadmap.md` and `docs/rig.md`.
Continuing without Claude: `docs/opencode-strata.md`.

## What this is
`atlh` runs F-Droid apps under our **hard fork of Android Translation Layer**
(ATL), classifies failures, ranks them, gates on regressions. The fork is
fixed against those results. Nothing ever goes to upstream ATL (AGENTS.md).

## Machines (`runners.toml`, `scripts/rig list`, `scripts/rig doctor --all`)
| Runner | What | Reach | Notes |
|---|---|---|---|
| `bazzite` (default) | x86_64 Bazzite box, Fedora 44 distrobox `atl`; also runs **strata** (local LLM, :8080) | ssh `atl-box` (192.168.10.177) | gates run here (16 cores, `--jobs 8`) |
| `mini` | aarch64 Fedora 44 Lima VM on the M4 Mac mini | ssh `atl-mini-vm` via ProxyJump `atl-mini` (192.168.10.160) | run/screenshot target only, never agents. Lima regenerates the VM's host keys on every VM start: re-pin after verifying via `limactl shell atl -- cat /etc/ssh/ssh_host_ed25519_key.pub` on the mini |
| `oneplus12` | the owner's OnePlus 12 (Fedora 45 from `fedora-atomic-phone`), USB networking | ssh `atl-oneplus12` (172.16.42.1, user `phone`) | everything lives in `/home/phone`: rig rootfs `~/atl-root` via bwrap. Follow the phone project's safety rules. `sudo` needs the owner's password in a real terminal |

ssh to runners is multiplexed (`ControlMaster` in `~/.ssh/config`): fast, and
avoids the OpenSSH per-source penalties that locked the mini's VM out twice.

## Repos
| Repo | Where | State |
|---|---|---|
| atl-harness (this) | github `Alaxend0l/atl-harness`, private | `main` pushed |
| ATL fork | github `Alaxend0l/andrun` (renamed 2026-11-23, D19; old path redirects), private; workstation clone `~/Develop/android-translation-layer` | `main` = `7cb4cd2b`; upstream fetch-only. Runners pull with read-only deploy keys (alias `github-atl-fork`). Beware: several agents share this clone — one loop switches branches in it live (it also checks out `main` in `~/Develop/atl-shaders` and keeps a detached scratch worktree under `/tmp/claude-*/scratchpad/atl-main`); it silently discarded uncommitted edits once this session. Commit WIP early; never rely on uncommitted files surviving. |
| fedora-atomic-phone | github `Alaxend0l/fedora-atomic-phone`; clone `~/Develop/fedora-atomic-phone` | branches **`plasma-mobile-spectacle`** and **`oneplus12-flatpak-home`** pushed, not merged |

## ATL fork: what's merged on `main`
All gated with the corpus (52 apps on bazzite) unless noted:

**Update 2026-11-22**: items 15–26 (the `local-fixes` line, tip `cd4d0a78`)
merged into `main` as `4a97b41b` on the owner's go-ahead, gated by full-corpus
`fc26` vs `fc25` (74 apps, 0 regressions) with both runners installed at
`local:cd4d0a78`.
1. SoundPool `load(String,int)`, 1-based IDs, load callbacks; `setShowWhenLocked`/`setTurnScreenOn` (Fossify Camera no longer crashes; blank = no camera).
2. Exceptions from app code: lifecycle → uncaught handler + exit 10 like Android; callbacks → logged as `W/ATL: swallowed exception in <fn>`, cleared (was: CheckJNI aborts). +6 apps.
3. TextView honours `android:gravity`/`setGravity` (Calculator labels centered).
4. Drawables: alpha through layer/ripple drawables; cached drawables no longer shared between views (Calculator digit/operator shades).
5. Password masking, input types, `InputType`, `Replacement`/`SingleLine`/`HideReturns`TransformationMethod. **Masking not yet seen on screen** (no corpus app shows a password field at startup; harness can't type yet).
6. EditText `getText()` returns a live Editable (edits reach the field); Enter sends the IME action + a proper key-down; CheckBox/RadioButton/Switch `setOnClickListener` works; CompoundButton `getText()`.
7. SharedPreferences change listeners fire; selectors inside layer-list/ripple get state changes (pressed/checked/disabled looks).
8. `Canvas.drawRect` honours stroke style.

9. EditText getText()/getEditableText() no longer call each other (infinite recursion hung Fossify Music at "Loading files", found by the owner).
10. **Media library** (`doc/MediaLibrary.md` in the fork): content://media answers from a real index (SQLite, MediaStore columns) of the XDG music/pictures/videos/download dirs + app storage; tags via libavformat (new native dep); albums/artists/genres/album art; insert/update/delete; MediaScannerConnection and MediaMetadataRetriever real. Was: every query opened a file chooser. Fossify Music lists albums and tracks, Fossify Gallery shows folders. Also: Parcel passes binders by reference, `Binder.equals` is identity, `ATL_BINDER_TRACE=1`, `getPackagesForUid` for the app's uid, `IBinder.isBinderAlive` etc.
11. AlertDialog buttons with a null listener dismiss instead of crashing (the local agent's fix, cherry-picked).
12. Native shaders: gradients, bitmap/compose shaders, affine canvas transforms (`doc/Shaders.md` in the fork; Claude): `<shape><gradient>` renders (was solid black), Cairo-rendered bitmaps no longer turn pink under transforms. The pink Fossify Music/Gallery covers were Fossify's *error placeholder* — fixed by item 14.
13. **Media session trust / uid identity** (Fossify Music now plays a track):
`UserHandle.getCallingUserId()` returned a hardcoded 1000, so
`generateApplicationInfo()` mangled the uid to `getUid(1000, -1)` = 99999999
while `Binder.getCallingUid()`/`Process.myUid()` say -1 — media3's
`isTrustedForMediaControl` then refused control and `MediaController` got
read-only player commands (`MCImplBase` warnings). Now
`getUserId(Binder.getCallingUid())`. Plus media3 1.11.1 `AudioTrack` init:
`ActivityManager.isRunningInTestHarness`, `AudioTrack.Builder.setSessionId`,
`getChannelCount`/`getUnderrunCount`/`getEncoding`/`getBufferSizeInFrames`/
`getLatency`, `AudioRouting` + routing listeners. Gate
(`fossify-base2 -> fossify-uidfix2`): 4 improved (camera, filemanager,
messages, musicplayer `CRASH -> ALIVE+ok`), 0 regressed.

14. **`View.layout()` calls `onSizeChanged()` and dispatches
`OnLayoutChangeListeners`** (`fix/albumart`, merged as `7cb4cd2b`): ATL only
did that from `layoutInternal()` (the GTK entry), so views sized by
Android-side ViewGroups (RecyclerView/ConstraintLayout call `layout()` on
their children) never notified their listeners — Glide's `SizeDeterminer`
waited forever on its one-shot listener and album/track covers stayed at
Fossify's pink error placeholder. Now real covers in Albums/Tracks/
now-playing (mini, run art16). Gate: bazzite `sbx1 -> fix2`: 0 improved,
0 regressed (see merge commit message). Two new `W/Canvas STUB:
drawColor(0x00000000, CLEAR)` signatures appear because Glide transforms now
run — `Canvas.drawColor(int, PorterDuff.Mode)` is still a stub, queued in
triage.md. Deliberately NOT called from `layout()`: `onLayout()` — the GTK
pass already runs it, and double-dispatching RecyclerView made RunnerUp go
BLANK (found by the gate, fixed before merge).

15. **Overnight-loop batch, staged on `local-fixes` (tip `2ea23604`), 2026-11-22**:
    18 audit fixes from loop `20261121-185746` (branch `agent/overnight`),
    all reviewed
    against their fix_hints (no fake fixes; the two lint flags — Settings
    `return null`, selectDrawable `return false` — are correct Android
    behaviour): setPaddingRelative; ImageView setImageBitmap/setImageResource
    tint path + setColorFilter; Color.colorToHSV/HSVToColor; attach-state
    listener list; TextView compound drawables (XML attrs + setters);
    Context.getColor(theme); ArgbEvaluator singleton; Path.addCircle;
    StateListDrawable theme inflate; Theme cache key; TextPaint copy
    ctor/set; NetworkCapabilities VALIDATED + NetworkInfo.isAvailable;
    Settings null-not-sentinel + hex android_id; selectDrawable(-1) clears;
    ViewPropertyAnimator one-shot actions; Button/EditText setText(BufferType);
    PropertyValuesHolder evaluator (task 20, finished by hand: the loop died
    mid-gate, its run `agent-20261121-185746-20` is unusable — ATL was
    restored to main mid-run — redone as `-20b`). Gate chain
    base->-19->-20b on bazzite: 0 regressed at every step. The two loop
    rejections were re-done by hand and staged on the same branch: ProgressBar
    `android:max/progress` via a non-virtual `applyInitialRange()` hook
    (`c3b2684b`, gate `pbmax1`; the loop's constructor-virtual-call variant
    → ClassCastException in Material's LinearProgressIndicator, run -12,
    trap now documented in `local-queue.json`) and
    DrawableContainer.setBounds (`2ea23604`, gate `dc1`; the loop's
    musicplayer CRASH was a SIGABRT on the Jit thread pool with no drawable
    frames, did not reproduce in 4/4 reruns — the known flaky-abort class).
    Owner instruction 2026-11-22 (D17): none of this goes to `main` without
    owner review — `main` was rewound to `7cb4cd2b` and all 20 fixes now live
    on `local-fixes`; merge decisions there are the owner's.

16. **Fixcov batch: the 9 fixcov crashes, staged on `local-fixes` (tip
    `44897c0d`), 2026-11-22** — 14 commits from branch `fix/fixcov-crashes`
    (worktree `~/Develop/atl-fixcov`), one full-corpus gate per step:
    `fcbase -> fc01` 6 improved / 0 regressed (tickmate, omninotes, siggen,
    tuner, kdeconnect `CRASH -> ALIVE`, diary `CRASH -> BLANK`),
    `fc02 -> fc03` and `fc03 -> fc04` 0 regressed, `fc01 -> fc02` +1
    (gramophone), `fc03 -> fc04` +1 (editor `CRASH -> BLANK`),
    `fc04 -> fc07` 0 improved / 0 regressed (the last two commits are
    outcome-neutral on x86_64).
    Fixes: `setContentView` inflates into the window content ViewGroup with
    `attachToRoot=true` (diary `<merge/>`); `ListActivity` binds
    `@android:id/empty` in `onContentChanged()` and forwards
    `onListItemClick` (tickmate — AOSP's `AdapterView` never auto-binds the
    empty view, the fork had the `setEmptyView()` call commented `// FIXME`);
    `ViewGroup.LayoutParams(LayoutParams)` + `copyFrom` (omninotes); real
    `android.app.ActionBar` + `Window.getActionBar()` on a shared
    `Toolbar` (`internal R.id.action_bar`) (billthefarmer tuner/siggen/editor);
    `MediaSessionManager.getActiveSessions()` +
    `OnActiveSessionsChangedListener` + the `media_session` service
    (kdeconnect — ATL has no media-session broker, said in the code);
    `RotateDrawable` + `<rotate>` in `Drawable.createFromXmlInner` (zhanghai
    — `Drawable.setLevel()` is a final no-op in ATL so the level arc cannot
    animate, documented); `StorageVolume` getters
    (`getUuid/getState/getPathFile/getDirectory/isEmulated/
    getMediaStoreVolumeName`) that MediaStoreCompat reflects on (gramophone);
    `View.getFitsSystemWindows()` keeps real state parsed from
    `android:fitsSystemWindows` (was hardcoded `true`, which made Gramophone's
    `enableEdgeToEdgePaddingListener` assert and abort);
    `ScrollView`/`HorizontalScrollView.smoothScrollTo/smoothScrollBy` +
    `computeScroll()` with an `OverScroller` (diary; the framework declares
    these on the scroll views, not on `View`);
    `android.widget.SearchView` + `android.view.CollapsibleActionView`
    (billthefarmer editor — verifying any class that implements
    `SearchView.OnQueryTextListener` aborted at activity construction);
    and `api-impl/meson.build` must list new classes: an unlisted file that
    nothing references is **silently left out of api-impl.jar** — SearchView
    was in the tree, built, and still `ClassNotFoundException` until listed
    (RotateDrawable had only slipped in because `Drawable` references it).
    Result on the 19 fixcov apps vs `main` (`fixcov-main` → `fc04`): 8 of the
    9 crashes gone (7 `ALIVE`, diary and editor `BLANK` — both are empty
    content, not a failure: the diary has no entries in a fresh data dir and
    the editor opens an empty buffer). Still broken: zhanghai Files (NPE in
    R8-obfuscated app code, `ap3.a`) and phonograph intro (appintro
    `AppIntroViewPager` count desync) — both queued with evidence in
    `local-queue.json`, together with the menu action-view gap
    (`MenuItemImpl.expandActionView` throws `UnsupportedOperationException`).
    The last two commits, both outcome-neutral in the gates: the main activity
    now gets an Intent with `ACTION_MAIN` + `CATEGORY_LAUNCHER` (it was
    `ACTION_MAIN` with no category, so `filterEquals(Intent.CATEGORY_LAUNCHER)`
    comparisons fail — found while reading the diary crash, and `Settings
    → Applications → Application details` needs it); and
    `ScrollView`/`HorizontalScrollView` create the `OverScroller` on demand
    (`ensureScroller()`), because ATL reaches `computeScroll()` from
    `WrapperWidget.c`'s size-allocate callback while the Java constructor chain
    is still running — on **aarch64 only**, where gramophone's now-playing
    header crashed with `NullPointerException @
    android.widget.HorizontalScrollView.computeScroll` (the x86_64 gates never
    hit that ordering, which is why the aarch64 run was worth doing).

    aarch64 verification (runner `mini`, 67-app DB after fetching the fixcov
    APKs there): `mfcbase` (build `local:f5753053`) -> `mfc01`
    (`local:1aed7fe5`) **8 improved / 0 regressed** — the same eight apps and
    the same outcomes as x86_64; `mfc01 -> mfc03` (`local:44897c0d`)
    0 improved / 0 regressed and the `computeScroll` NPE is gone (checked in
    the `failures` table, not just outcomes). Fixcov on mini at the tip: 6
    ALIVE, 2 BLANK (diary, editor), `me.zhanghai.android.files` CRASH,
    `player.phonograph.plus` EXITED — identical to bazzite. New mini-only
    signatures still open on gramophone: `ClassNotFoundException:
    android.media.MediaFile` (real missing API, used by its MediaStore
    deletion path) and `NoSuchFieldException @ android.view.View.layoutInternal`
    (kotlin-reflect on a private field), queued.
    Partial also: audit M72/M74 (the scroll views now animate, but
    `fillViewport`, `fullScroll` and the GtkScrolledWindow mapping are still
    missing, so those audit items stay open in `fixed.txt` terms).

 17. **Phone runner (`oneplus12`) brought to the fixcov tip + corpus,
     2026-11-22** — USB networking is not coming up from this workstation
     (the phone enumerates once as the `fedora-atomic-phone` "ramboot debug
     shell" gadget, then `device descriptor read/64, error -71`), so
     `~/.ssh/config` now points `atl-oneplus12` at `waffle.local` (LAN, same
     sshd; docs/setup.md). `rig -r oneplus12 build --worktree
     ~/Develop/atl-fixcov` installed `local:44897c0d` (was
     `local:7cb4cd2b-dirty`); `atlh fetch --pkg-file corpora/fixcov.txt` added
     the fixcov APKs (DB 35 → 54 apps, 691 MB). First run (`pfc01`) lost 11
     of 19 apps as `CRASH (0 signatures)`: the phone has no `/dev/net/tun`, so
     `atl-sandbox` cannot start pasta (`docs/sandbox.md`) — with
     `--no-sandbox` (`pfc02`) the phone matches mini: 15 ALIVE, 2 BLANK
     (diary, editor), `me.zhanghai.android.files` CRASH, phonograph EXITED.
     Phone-only: `org.billthefarmer.siggen` **SIGSEGV** (`SEGV_MAPERR` fault
     addr 0x20, thread `pool-1-thread-1`, no backtrace printed) where mini and
     bazzite say ALIVE, and phonograph's EXITED signature is
     `IllegalStateException @ android.app.Activity.nativeStartActivity`
     instead of the PagerAdapter one — both queued. Side finding while
     checking the phone's dex: ATL dexes with `dx --incremental`, so a runner
     that builds repeatedly keeps stale class defs in `api-impl.jar` (mini 9.2
     MB / 1710 defs vs the phone's clean 3.3 MB / 1696 defs) — harmless for
     lookups, but jar size is not a usable sanity check.

On the phone: ATL `local:44897c0d` (the fixcov tip, 2026-11-22; see item 17)
is installed; `rig -r oneplus12 doctor` shows the current commit. Note for
fork commits: the build regenerates `src/api-impl-jni/generated_headers/*.h`
with `javac -h`; commit the regenerated header with any new native method.

 18. **`scripts/atl-gaps` + queue refill + 4 more queue fixes (gate
     `fc07 -> fc08`), 2026-11-22**: `scripts/atl-gaps` (docs/usage.md) is the
     static counterpart of `atlh missing-apis`: it classifies every method
     body in `src/api-impl` (empty/unsupported/const-return/log-only/log+const/
     stub-call/commented-out/native-missing/todo/print-in-body/hook) and joins
     each gap with runtime hits (sqlite), corpus dex references, and
     queue/fixed dedupe; `--emit-queue N` refills `local-queue.json`. Scan of
     `44897c0d`: 3390 reported gaps (empty=1631, const-return=812,
     native-missing=410 — of which 307 are GLES declarations the C layer does
     not implement, unsupported=125…); 3383 have **no runtime coverage**, so
     fetching apps that use them is the prerequisite for most of it
     (`atl-gaps.json/md` saved under git-ignored `work/gaps/2026-11-22/`).
     Refilled the queue with its top 10 (now 78 items). Fixed the first four
     live queue items on `fix/fixcov-crashes` -> `local-fixes` tip `7f906b29`
     (pushed, `main` untouched): `<include android:visibility>` (LayoutInflater
     parseInclude), Intent copy-ctor/putExtras Bundle copy+merge (getExtras
     still returns the live bundle — noted in the commit), View scrollTo ->
     onScrollChanged + OnScrollChangeListener, Window setFlags/addFlags/
     clearFlags on the window attrs + `FLAG_KEEP_SCREEN_ON=0x80` (was 0) +
     5-arg LayoutParams ctor storing type/flags/format. Gate `fc07 -> fc08`:
     0 improved / 0 regressed (outcome-neutral by nature; proves no breakage).
     `fixed.txt` now also covers those four, so the overnight loop skips them
     (queue audit: items 0–20 were already fixed on the branch, the queue
     just predates the fixes).

 19. **Fixes verified by their own crash signatures, and the first
     fetch-an-app-for-a-gap batch (gates `fc09`–`fc12`), 2026-11-22**:
     new AGENTS.md hard rule: if a gap has zero corpus hits, fetch an
     F-Droid app that does hit it and A/B, otherwise the gate proves
     nothing. On `fix/fixcov-crashes` -> `local-fixes` tip `c75de5c0`
     (pushed, `main` untouched):
     - queue items 25+26 (`599f54b7`): AudioManager ringer mode
       NORMAL + per-stream volume table; AccountManager 3-arg
       addOnAccountsUpdatedListener + listener list, new
       `android.content.SyncStatusObserver` (added to meson.build srcs,
       confirmed in the dex by string count), ContentResolver
       SYNC_OBSERVER_TYPE_* (AOSP 1<<0..1<<3) + add/removeStatusChangeListener
       (no sync service in ATL: the handle never fires, commented).
       Gate `fc08 -> fc09` 0 improved / 0 regressed, but org.tasks'
       three AccountManager/SyncStatusObserver signatures were gone.
     - `de7a228b` LocationManager.removeUpdates(PendingIntent) (AOSP
       null->IllegalArgumentException; nothing to remove since ATL never
       registers PI updates) + removeUpdates(LocationListener) finally
       removes from the set. Gate `fc09 -> fc10` 0/0; org.tasks moved
       to the next cause.
     - verification batch `corpora/verify-batch.txt` (fossify.clock,
       best.deskclock, antennapod, davdroid, k9; 3 newly fetched):
       A/B `vbase` (build `44897c0d`) vs `vtip` - same outcomes, so
       fixed.txt got 25/26 and the loop can skip them.
     - `e5a75917` Context.registerReceiver(receiver, filter, flags)
       (API 33 variant; ATL broadcasts are app-local);
       `cc1dc294` Settings$System.DEFAULT_ALARM_ALERT_URI +
       RingtoneManager TYPE_* and getDefaultUri mapping (AOSP
       circularity, ATL's provider URIs) - **com.best.deskclock
       CRASH -> ALIVE with 0 signatures** (the fetched alarm app is
       the witness for the item-25 audio area);
       `c75de5c0` IntentFilter.dump(Printer,String): AOSP has no
       dump on IntentFilter; the call site resolves to the **void**
       descriptor (`dump(ZLL)` in the dex was not enough - ART said
       so; verified with a minimal dex parser, which also showed the
       incremental builddir keeping stale defs next to new ones).
       Gate `fc10 -> fc11` (74 apps incl. the new batch): **org.tasks
       CRASH -> ALIVE**, 0 regressed. Gate `fc11 -> fc12`: 0/0, the
       dump signature gone, org.tasks 3->2 signatures.
     - davdroid stays CRASH on its own provider class (queue:
       `at.bitfire.davdroid ... DavDocumentsProvider`
       needs_investigation) plus `getRestrictBackgroundStatus` (queue);
       K-9 ALIVE (2 sigs), Antennapod/Fossify Clock ALIVE.
     Queue now 80 items; fixed.txt covers 25/26 too.

 20. **davdroid fixed: the missing superclass was the whole story
     (gates `fc12` -> `fc13`), 2026-11-22**:
     - `9b996923` queue-78 `ConnectivityManager.getRestrictBackgroundStatus`
       (+ RESTRICT_BACKGROUND_STATUS_* constants, AOSP values; DISABLED
       since ATL has no data saver).
     - `4e33640b` queue-27+30 framework `PopupMenu` Menu/MenuItem surface:
       all UnsupportedOperationException stubs implemented (groupId/order/
       checkable/... stored; removal always via setVisible(false) so the
       GMenu visible-position bookkeeping stays consistent; item click
       order itemListener -> intent -> popup listener). Honest scope: no
       corpus app crashes on these (no PopupMenu/"Unimplemented method"
       signature anywhere in fc12/fc13), 29 apps reference the class;
       gate shows no-breakage only, no witness crash exists.
     - `3a438448` `android.provider.DocumentsProvider` (queue-79):
       investigation per the task hint first — davdroid's own class IS in
       its classes.dex (13 refs), ATL's api-impl dex has ZERO refs to
       `android/provider/DocumentsProvider`, and `ATLLoadedApp.loadClass`
       is plain `Class.forName`, so ART fails to *define* the app class
       (unresolvable superclass) and blames the app class name. davdroid's
       dex implements exactly the classic provider surface. Ported AOSP-14
       DocumentsProvider (+ meson srcs entry — dex-verified after the
       known silent-drop trap), with ATL adaptations commented: no
       URI-permission enforcement in call() (single-process, Context
       grant/revoke are log stubs), no canonicalize, the Bundle-query
       dispatch lives in the 5-arg query() ATL actually has, Log.wtf->Log.e.
       Support bits also AOSP-faithful: DocumentsContract.EXTRA_TARGET_URI/
       buildTreeDocumentUri/Path, ClipDescription.compareMimeTypes,
       ContentResolver.EXTRA_SIZE, ContentProvider.getStreamTypes/
       openTypedAssetFile bases.
     - Gate `fc12 -> fc13` (74 apps): **at.bitfire.davdroid CRASH -> ALIVE**,
       0 regressed; CNFE count 0, provider loads; the fetched witness-app
       rule is satisfied with an outcome flip, not just no-breakage.
     Queue items 27/30/78/79 now in fixed.txt. Remaining davdroid
     signatures are inert stubs + the shared ViewGroup.native_addView
     exception (separate item). Live queue: Vibrator (28), ContentResolver
     observers (29), then the low-impact tail.

 21. **Silent-behaviour batch: Vibrator + ContentObservers
     (gate `fc14`), 2026-11-22**:
     - `e9265516` queue-28 `Vibrator.vibrate(VibrationEffect[,
       AudioAttributes])` + `VibrationEffect` storing timings/repeat/
       duration (+ createPredefined/DEFAULT_AMPLITUDE/getDuration);
       repeat index now loops from `repeat` until cancel, and a new
       pattern cancels the old one. hasAmplitudeControl false (native
       input-event vibrator is duration-only).
     - `755e604d` queue-29 `ContentResolver` observer registry with
       AOSP URI-tree matching (ancestor+descendants flag, observers
       below the changed URI always notified, deliverSelfNotifications,
       null-uri matches all, copy-before-iterate), `notifyChange(uri,
       observer, flags)` overload, and ATLMediaContentProvider now
       firing notifyChange after MediaStore insert/update/delete.
     Verification honestly recorded in both commits: 48/60 of 74 apps
     reference these APIs in dex but none crashed on them, so no
     witness signature exists - gate `fc13 -> fc14` 0/0 (74 apps) is
     no-breakage only; the runnerup.free 3->4 signature count blip is
     stub-log noise (AssetManager.destroy), same real signatures both
     sides. Methods dex-verified. Bazzite installed `local:3a438448-
     dirty` (= `755e604d` tree).

 22. **Biggest shared signature killed: `native_addView` NoSuchMethod
     (gate `fc15`), 2026-11-22**:
     - `083e0c4e` `View.getViewRootImpl()` + minimal `ViewRootImpl` +
       `View$AttachInfo` (new meson srcs entry). The signature
       `NoSuchMethodException @ android.view.ViewGroup.native_addView`
       (15/74 apps in fc14 — fdroid, k9, tasks, kdeconnect, davdroid...)
       is androidx WindowInsetsCompat$Impl20.loadReflectionField failing:
       it wants `getViewRootImpl` + `View$AttachInfo.mVisibleInsets` +
       `ViewRootImpl.mAttachInfo`, all absent in ATL. Handled by androidx
       (apps stayed ALIVE) but counted as a failure. Semantics kept
       AOSP-faithful: null for detached views; attached views get the
       per-window stand-in whose visible insets are the whole window
       frame (ATL windows have no system bars — exactly the state AOSP
       reports that value for).
     - Runtime-verified, no fetch needed: gate `fc14 -> fc15` (74 apps)
       removed the signature from all 15 apps (15 -> 0; corpus total
       147 -> 127), `Failed to get visible insets` in the fdroid log
       1 -> 0, 0 regressed, zero new signatures in those apps.
     - Queue item 59 (dispatchApplyWindowInsets/WindowInsets surface) is
       adjacent but NOT done — this fix covers only the reflection chain.

 23. **Queue tail batch 31–37 + interpolator crashes (gates `fc16`–`fc18`),
     2026-11-22**:
     - Queue refresh: `atl-gaps --emit-queue 20` against the fixed tree
       (queue now 100 items; no runtime-hit gaps remain — all were
       fixed/filed).
     - Small fixes, one commit each: `0ff2c21a` queue-31 ProgressBar
       setIndeterminate (parameter shadow), `c64c7aab` queue-32
       SharedPreferences putX(key,null) removes, `8e7f67bc` queue-33
       CountDownTimer first tick immediate, `96c45b78` queue-34
       peekDecorView returns the decor view, `e3c20343` queue-35
       moveTaskToBack/setFinishOnTouchOutside public, `b4d0f3ce`
       queue-37 IntentFilter.match early NO_MATCH_ACTION (was: every
       filter matched data-less intents). All silent-behaviour: gate
       `fc15 -> fc16` 0/0, no-breakage only, recorded honestly.
     - `ed3f2760` + `7ccb4a77` queue-54 interpolators:
       `BounceInterpolator` and `AnticipateOvershootInterpolator`
       (AOSP-14 fetch; first draft used the pre-14 ctor set/formula and
       the witness then failed on `<init>(FF)V` — fixed by porting the
       actual android14-release source). Crash witness
       indi.dmzz_yyhyy.lightnovelreader's static-init chain: gates
       `fc15 -> fc16` (signatures gone) then **`fc17 -> fc18`:
       CRASH -> ALIVE**, 0 regressed. Remaining lightnovelreader
       signatures (SSL handshake, forkAndExec) are environmental.
     - Commits pushed; `local-fixes` = `7ccb4a77`.

 24. **Fossify Music media-notification crash (gates `fc19`–`fc20`),
     2026-11-22**:
     - Source was NOT the queue: the user ran `org.fossify.musicplayer` on
       the phone desktop (19:02); the launcher log
       (`~/.local/state/atl/org.fossify.musicplayer.log`) showed ATL
       swallowing `NullPointerException @ Bundle.putParcelable` — on real
       Android the app dies. Decompiled the APK frames with androguard
       (venv `/tmp/dexvenv` on bazzite): `iq2.g` reads
       `Notification.extras` and putParcelables the `"android.mediaSession"`
       token — the androidx.media `MediaNotificationManager` pattern.
     - Repro attempts on bazzite (scratch `work/config-mp.toml` +
       `work/interactions-mp/`, runs `mpnpe1`/`mpnpe2`, play+pause+next)
       start playback but never reach that callback path pre-fix — the
       trigger is desktop-session-specific (likely the controller
       callbacks tied to the session bus/MPRIS bridge). Crash witness =
       the phone log; bazzite evidence is the unblocked deeper path.
     - `3b241aa9` Notification: constructor and `Action()` create `extras`
       (AOSP `init()`), `setExtras` replaces / `addExtras` merges (were
       empty stubs). Gate `fc18 -> fc19`: 0 regressed; musicplayer
       signatures 2->6 — the unblocked chain exposed
       `NoSuchMethodError: Context.startForegroundService`.
     - `a0ac74ca` `Context.startForegroundService` (abstract on `Context`,
       `ContextWrapper` delegates, `ContextImpl` -> `startService`; AOSP's
       ActiveServices foreground timeout has no ATL counterpart, commented).
       Gate `fc19 -> fc20`: 0 regressed; musicplayer 6->4 signatures — the
       NoSuchMethodError and the swallowed-exception rows are gone.
     - Phone A/B pending: after the next phone build, a Fossify Music play
       in the desktop session should show no NPE in its log.
     - Follow-ups: atl-gaps never queued either bug (missing field init +
       empty builder stubs are invisible to it) — a scan for public
       reference fields never assigned in constructors would help; queue
       item 61 (NotificationChannel) is adjacent and still open.
25. **Gramophone crash → frozen UI → dead playback chain (gates
     `fc22`–`fc24`), 2026-11-22** — the owner ran Gramophone on the phone
     desktop; three stacked causes, fixed in order, each with a witness:
     - `22111b88` `MediaSession.sendSessionEvent` was missing: the moment
       androidx.media3 1.8.0 posted a session event Gramophone died with
       `NoSuchMethodError` (phone desktop log). A no-op + log matches what
       AOSP does for an abandoned session. Gate `fc21 -> fc22`: 0
       regressions.
     - `465bb53b` `android.media.Rating` port (concrete styles as static
       inner classes + `newHeartRating(boolean)`): its absence made the UI
       freeze after the crash-handler path ("Music works now, excellent" —
       owner confirmed on the phone). Gate `fc22 -> fc23`: 0 regressions.
     - Skip-to-next showed the next song but never played it, and the seek
       bar died. Root causes (bazzite scratch runs `gpost*`, ALSA null
       device for reproducible timing, backup evidence `work/remote/
       bazzite/runs/gpost7`): (a) `AudioTrack.write` on a **paused** track
       returns 0 forever under ALSA (Android buffers it) → media3
       `handleStalledWrite` spins on 40-frame silence, playback thread
       dead; (b) `flush()`/`stop()` were stubs and the head position could
       go negative (`head w=14048 delay=16096`); (c) `start_threshold` =
       whole buffer delayed consumption until media3's watchdog fired
       `audioTrackUnderrun`; (d) a failed `snd_pcm_open` (phone ssh session,
       "Host is down") kept a garbage handle → **SIGSEGV on the ExoPlayer
       thread** (phone scratch run `pgram0`). Fixed in `b858fdff` (buffered
       paused writes, `flush`/`stop` via `native_flush`=`snd_pcm_drop`,
       head clamp, `STATE_UNINITIALIZED` on open failure + NULL guards).
     - The other half of "shows the next song but doesn't start it" was in
       the **app**: cloning the source (github.com/FoedusProgramme/
       Gramophone, tag `1.1.2` = versionCode 24 — clone the app's listed
       repo before decompiling, AGENTS.md) shows `onTracksChanged` calls
       `controller.stop()` whenever Tracks are non-empty but *unselected*;
       with no FLAC decoder in ATL's `MediaCodecList` a skip onto the FLAC
       test album could never select a format, so Gramophone itself stopped
       the player (witness `gpost11` + thread dumps: playback looper idle,
       codec released, nothing rebuilt). `9690cf54` adds flac (ffmpeg has
       the decoder; `configure()` routes to `native_configure_audio`).
       Verified headlessly: `gpost12` shows the full skip rebuild
       (configure flac → new AudioTrack → play), zero underrun/stalled
       signatures, screenshots show Beta Tune 1 playing; pause/resume
       re-prepares cleanly. Gate `fc23 -> fc24` (19 fixcov apps) and
       full-corpus `fc23 -> fc25` (74 apps): 0 regressions both.
     - Phone rebuild + owner re-test of skip/seek on the desktop is
       pending; queue also picked up the `MediaStore$Audio$Playlists` CNFE
       seen in the phone Gramophone log.
26. **Gramophone on the phone: resume symptom, a real SIGSEGV, FLAC
     stutter (gate `fc26`), 2026-11-22** — owner's re-test after item 25:
     "Skipping successfully moves to the next song but doesn't play it,
     pressing play makes the song play, pressing pause makes the song stop,
     but pressing play after this one more time does not start the song
     again", and later "Coalescence by Taishi is stuttering during playback"
     (FLAC 44.1/16 stereo; opus "Cubensis Lenses" and Fossify Music Player fine).
     - The resume part did **not** reproduce as a resume bug: scripted
       play→pause→resume on the phone with temporary ALSA-state probes showed
       clean transitions (pause → `PAUSED`, resume → `start=0` → `RUNNING`;
       the `unpause=-77`/EINVAL is the unconditional `snd_pcm_pause(FALSE)`
       after a successful start, harmless). What the probes did find is the
       *crash* that had been masquerading as it: `Error calling snd_pcm_open:
       Host is down` then `Fatal signal 11 (SIGSEGV), fault addr 0xbd10,
       Thread: "ExoPlayer:Playb"` — both `AudioTrack.getMinBufferSize` and
       `AudioRecord.getMinBufferSize` logged the open failure and then probed
       the unopened `snd_pcm_t *` (the constructors had the same shape:
       garbage `period_size` feeding `sw_params`, and AudioRecord dividing by
       zero). Fork `9e4abf80` returns Android's negative error / no-handle and
       closes the handle. Witness on the phone runner: same script, `pres2` =
       CRASH (`signal: SIGSEGV`) → `pres3` = ALIVE with no `Fatal signal`.
       bazzite cannot reach that path (its ALSA device never refuses an open).
     - **FLAC stutter: open, two leads** (queue items 103–104). (a) The
       `MediaCodec` Java shim hands the decoder four 16384-byte buffers and
       `native_dequeueOutputBuffer` `swr_convert`s into one with no capacity
       check — a 4608-sample FLAC frame is 18432 bytes, so the tail of every
       frame is lost and the write runs off the end of a Java heap array (the
       owner's album uses 4096-sample blocks = exactly 16384, zero margin).
       (b) In the scripted repro (runs `coal3`/`coal4`, `work/config-coal.toml`
       + tap on the Coalescence play icon at 64 401) media3 tore its sink down
       3× with `AudioSink$InitializationException: AudioTrack init failed 0
       Config(44100, 12, 2, 44100)` — sink rebuild churn is audible as
       stutter, and there `AudioTrack.write` was never reached. For whoever
       picks it up: the phone's PCM device is ATL's own
       `alsa_playback.android-translation-layer` plugin, and the harness keeps
       only a trimmed log, so read `~/.local/state/atl/
       org.akanework.gramophone.log` from the last `=== <ts> atl-app` marker.
     - Also landed (not audio): `cd4d0a78` real
       `AccelerateDecelerateInterpolator` (AOSP cosine) and `PathInterpolator`
       (AOSP sampling + the `IllegalArgumentException` checks) plus a native
       GskPath-flattening `PathMeasure` replacing the `getLength() = 100` stub.
       Witness is numeric, not app-level: `fdroid.basic` builds
       `PathInterpolator(0f, 1.1f, 0f, 1f)`, sampled x=0.5 → 1.0013443 and
       x=0.25 → 0.9751814 (the analytic cubic; old stub gave 0.5). No corpus
       app is known to use `PathMeasure`, so a trim-path VectorDrawable app is
       still owed as its witness. `Path.approximate()` left for later.
     - Gate `fc26` (full corpus, 74 apps) vs `fc25` on bazzite: 0 regressed,
       0 improved; both runners installed at `local:cd4d0a78`. On that verdict
       the owner-authorized merge landed: fork `main` = `4a97b41b`
       (merge of `local-fixes`/`fix/fixcov-crashes`), mirror pushed.

27. **MediaCodec audio output-buffer overflow fixed (gate `fc27`), plus
     two audit fixes (gate `fc28`), 2026-11-22** — fix-loop session on
     `fix/fixcov-crashes` (= `local-fixes`).
     - Fork `709f7b66` (queue item "MediaCodec audio output: swr_convert
       writes into a fixed 16 KiB Java buffer with no capacity check"):
       the lead (a) from item 26 was a real bug. Audio codecs got four
       16384-byte output buffers while `native_dequeueOutputBuffer`
       `swr_convert`s a whole AVFrame in with no capacity check — a
       4608-sample stereo FLAC frame is 18432 B, so 2 KiB ran past a
       `GetPrimitiveArrayCritical` Java array and `BufferInfo.size`
       exceeded the ByteBuffer capacity, which media3 rejects. Witness
       pair on bazzite (scratch library `work/media-103`: stereo 44.1 kHz
       FLAC, 143 frames x 4608 samples; scratch config
       `work/config-103.toml`, tap the play icon at 67 300): pre-fix
       `mcpre4` = `IllegalArgumentException at java.nio.Buffer.limit` in
       `MediaCodecRenderer.drainOutputBuffer` → `playerFailed
       [mediaPos=0.00]`, AudioTrack never played; post-fix `mcpost1` =
       same script reaches `calling AudioTrack.play()`, zero
       Buffer/playerFailed lines. Fix = 32768-byte audio output buffers
       in MediaCodec.java (primary, witnessed) + capacity clamp and
       drain-on-next-dequeue in the C shim (defence-in-depth, no corpus
       witness — logged as "tail deferred" if it ever fires).
       Tooling follow-up owed: the gate's Beta FLAC album is *mono*
       4608-block (9216 B, fits) — make it stereo in
       `scripts/runner/make-test-media.sh` so the corpus covers this.
     - Queue 104 (phone FLAC stutter) finding while triaging: the
       `Config(44100, 12, 2, 44100)` in the coal4 sink failures is
       channelConfig 12 (stereo) + encoding 2 (PCM_16BIT) — **not** a
       24-bit encoding, an earlier suspicion was wrong. The reopens
       fail because ATL's own `alsa_playback.android-translation-layer`
       plugin intermittently refuses them ("Host is down"); initial
       trigger still unknown, repro needs the phone.
     - Fork `a9c384e1` (Resources drawable-cache `shouldInvalidateEntry`
       → `Configuration.needNewResources`, matching AOSP and this
       package's own ConfigurationBoundResourceCache) and `c5024620`
       (ViewGroup.offsetChildrenTopAndBottom offsets its children —
       GridView's seven gap-correction call sites were no-ops). Both are
       compile + no-regression verified only: no corpus app reaches
       those paths in the scripted gates (checked every run's signatures;
       ~20 APKs reference `updateConfiguration` but none hit the prune
       path). Real witnesses still owed.
     - Gates on bazzite: `fc27` (74 apps, tree `709f7b66`) vs `fc26`:
       0 regressed, 0 improved. `fc28` (74 apps, tree `c5024620`) vs
       `fc27`: 0 regressed, 0 improved.
     - Same session, queued behind fc28: fork `a3c8388a` (drop the
       per-frame `updateTintFilter` and PopupMenu debug prints) and
       `71b502f5` (TextView actually stores/returns/paints its hint —
       `getHint()` returned the literal "HINT", `setHint` was empty;
       EditText untouched, it uses the GTK placeholder). Gate `fc29` vs
       `fc28` (74 apps): 0 regressed, 0 improved.
     - Queue staleness note: item "SoundPool.load prints on every
       call" (index 73) is **stale** — no such print exists in the
       current tree (SoundPool was reworked earlier); skip it.
28. **Fix-loop marathon: queue items → `fc29`..`fc37`, 2026-11-22 night** —
     same fix loop, `fix/fixcov-crashes` (= `local-fixes`), all gates on
     bazzite, every gate 0 regressions.
     - 21 audit-queue fixes `940749c9..37a8a579` (see `git log`):
       drawable scheduling tokens, getSuggestedMinimum*, DateFormat,
       LengthFilter, onBackPressed print, attachViewToParent double-add,
       drawText(char[]) count, Context file API + clip + htmlEncode/
       ellipsize, MediaFile type table (witness: gramophone CNFE gone in
       `fc32`), NotificationChannel/Group getters, Toast state,
       deleteFile/ContentResolver cache + LogPrinter (witness:
       com.vermont.possin CRASH→ALIVE in `fc33`), state for
       description/focusable/delegate/tokens, Canvas point family,
       Drawable tint/filter + LayerDrawable propagation, WebViewClient
       signatures, OnApplyWindowInsetsListener dispatch.
     - Item 64 (phonograph intro ViewPager ISE): fork `37a8a579` —
       View.getWindowToken() returned a token from the first instant;
       AOSP returns null until the tree is attached after onCreate, and
       androidx ViewPager.populate() uses exactly that as its guard
       before the count check (viewpager 1.0.0 sources; AppIntro 6.3.1
       addSlide sets offscreenPageLimit before notifyDataSetChanged).
       ATL proxy: first window-driven (JNI layoutInternal) layout marks
       attached; getWindowToken walks the parent chain. Witnessed in
       `fc36`: the ViewPager IllegalStateException is gone, phonograph
       now crashes deeper (see next).
     - phonograph round 2 (found in `fc36`): "LifecycleOwner ... is
       attempting to register while current state is STARTED" —
       root cause: the fork never fired Fragment.onActivityCreated, and
       androidx lifecycle 2.11's (framework-)ReportFragment dispatches
       Lifecycle ON_CREATE exactly there; the registry jumped
       INITIALIZED→STARTED at onStart and replayed ON_CREATE *after*
       publishing STARTED, tripping ActivityResultRegistry.register's
       check inside phonograph's deferred launcher delegates. Fix
       `3afca7d6`: dispatchFragmentsActivityCreated() from C between
       onCreate and onActivityPostCreated, AOSP performCreate order.
     - Also fixed: item 83 Activity.setVisible state (`b0d7b392`),
       item 99 View.canScroll* + ScrollView compute* overrides
       (`91de247e`), item 50 ThumbnailUtils centre-crop (`abbfd3d5`) —
       all no-regression verified (no corpus app calls them at startup).
     - Stale / deferred queue items recorded in `docs/audit/fixed.txt`
       skip-lines rather than code: items 36/39/44–48/53/60/82 were
       already fixed by earlier commits (title lines added), 73
       SoundPool print gone, 68/69 ActionMode.setSubtitle stale (the
       fork's ActionMode methods are abstract, no concrete impl; the
       real gap is View.startActionMode returning null — bigger separate
       gap), 74 android.gesture GestureStore deferred (useless without
       the whole Gesture/GestureList/GestureLibraries framework), 66
       kotlin-reflect layoutInternal stale (no kotlin-reflect failure in
       any fc run's logs; fork View has the *method*, and AOSP has no
       such field either), 77 Theme.obtainStyledAttributes stale (it is
       fully implemented through `getAssets().applyStyle`, no TODO).
     - Gates: `fc30`..`fc36` (74 apps each) vs previous: 0 regressed;
       improvements: fc32 (gramophone MediaFile), fc33 (possin
       LogPrinter). `fc36` also progressed phonograph's crash signature
       without flipping its status (compare scores status only).
       `fc37` (tree `abbfd3d5`) vs `fc36`: **0 regressed, 2 improved** —
       phonograph plus EXITED→ALIVE (witnesses `3afca7d6`), and
       com.nitulshah.nudge CRASH→BLANK. The nudge flip is a *timing*
       side-effect, not its root cause: nudge's own class extending
       `android.accessibilityservice.AccessibilityService` still fails
       to resolve (fork has no such class); the exception now surfaces
       inside a swallowed callback instead of the lifecycle dispatch.
       Real fix = the accessibility-service class family — queued
       context here, not yet done.
     - After fc37, gated and clean: `fc38` (tree `00d1c8d1`: item 49
       VectorDrawable android:fillType parsed+applied via
       `Path.FillType.getMapFillType`, item 51 Ringtone.getTitle
       resolved through MediaStore with URI-name fallback) and `fc39`
       (tree `9a22a650`: item 96 Activity.dispatchKeyEvent real window
       chain + Window.superDispatchKeyEvent + AOSP KeyEvent.Callback
       defaults) — both **0 improved, 0 regressed** vs their
       predecessors (no corpus witness; no-regression only, said so in
       the commit messages). `fc40` (tree `38bc7b3e`: queue batch
       105/108/109-115/117/118 — View transform state, real focus
       state via new nativeHasFocus/nativeClearFocus JNI,
       isLaidOut/layerType/autofill/rootWindowInsets state): **0
       regressed, 1 improved** — `me.zhanghai.android.files`
       CRASH→ALIVE (witnesses the isLaidOut fix, see next). `fc41`
       (tree `5f900c6d`, TextView isSingleLine): 0 improved, 0
       regressed (the probe fix is inside an already-ALIVE app).
     - Queue batch 119–133 (View transform/clip/outline/window state,
       BoringLayout, Activity config/focus), fork `f10231e1`..
       `43519051`: rotation/rotationX/Y state; clipBounds +
       clipToOutline + outlineProvider state (invalidateOutline
       documented: ATL draws no outlines, nothing to recompute);
       nested-scroll flag, window system-UI visibility,
       requestApplyInsets re-dispatch, pointer icon state;
       BoringLayout.isBoring real detection (the fork's two variants
       lied in opposite directions); View.hasWindowFocus via new
       nativeHasWindowFocus (GTK4 `gtk_window_is_active`) + attach.
       **Gate `fc42` FAILED — first regression caught: item 131
       Activity.hasWindowFocus as AOSP-accurate state (initial false,
       JNI-set on every onWindowFocusChanged) regressed
       me.zhanghai.android.files ALIVE→CRASH.** Reproduced as a timing
       race (3 smoke runs: 1 crash on the committed tree, 6/6 clean
       with initial state true): an androidx path in the app (string
       in the dex, not in the app's open source) reads hasWindowFocus
       before ATL's first focus callback; false sent it into a branch
       re-running the item-63 loader chain against null members
       (bisected View-half vs Activity-half by targeted smoke). Fix
       `f5e48a44`: keep initial true (the value the fork always
       guaranteed), field stays real state afterwards. Gate `fc43` vs
       `fc41`: **0 improved, 0 regressed**. Item 132
       getChangingConfigurations recorded stale: ATL restarts no
       activities on config change and never dispatches
       onConfigurationChanged to them, so AOSP's 0 is faithful.
       (Note: `f5e48a44`'s message has a cosmetic "abe939e5..." typo
       for `abe937e5` — left as pushed.)
     - Queue batch 134–148 (fork `520304da` + `ab389a2c`): haptic
       feedback suppression flags (+ missing FLAG_* constants),
       descendant focusability (+ missing FOCUS_*_DESCENDANTS
       constants), over-scroll mode (default corrected to
       IF_CONTENT_SCROLLS), background tint, text direction,
       insets-animation callback state, getClipToOutline wired to the
       clip state, stopNestedScroll documented no-op (no nested-scroll
       machinery in ATL — ViewGroup's onNestedScroll* throwing is
       recorded as its own gap). Eight items recorded stale with
       reasoning (AOSP-identical defaults: 134/148; no config restarts
       in ATL: 145/146; input-model facts: 142; already-debug-gated:
       143; implemented-already: 135; a11y stub: 141). Gate `fc44` vs
       `fc43`: **0 improved, 0 regressed** (no witnesses; no-regression
       only).
     - Queue batch 149–163 (fork `fdda777c`..`0bfe2dd5`):
       getApplicationWindowToken now answers the same attach token as
       getWindowToken (was null); AutofillId rewritten to wrap a value
       with equals/hashCode and View lazily assigns a unique id per
       view (the class was **empty** and the getter made a fresh object
       per call, so no round-trip or equality could ever work); the
       hover chain wired up — `OnHoverListener`'s interface body was
       `// TODO` with **no method at all**, so `setOnHoverListener`
       stored a listener that could not be called: AOSP's
       `boolean onHover(View, MotionEvent)` added (app classes are
       compiled against it), listener stored, `dispatchHoverEvent`
       follows AOSP's listener-then-onHoverEvent chain (ATL still does
       not feed GTK hover into the view tree — input gap);
       `getFocusables` returns self when visible/enabled/focusable
       (AOSP base-View semantics; ViewGroup's descendant-collecting
       override does not exist in ATL — GTK owns focus search — noted
       as its own gap); `Intent.setExtrasClassLoader` forwards to the
       Bundle (which already keeps and uses mClassLoader);
       backgroundTintMode state; fading-edge length state + the
       previously absent getHorizontalFadingEdgeLength; getMeasuredState
       is now AOSP's composition over the measured sizes instead of a
       literal 0 (still 0 in practice, because ATL's measure path never
       sets MEASURED_STATE_TOO_SMALL — said so in the commit). Gates
       `fc45` vs `fc44` **and** `fc46` vs `fc45`: **0 improved, 0
       regressed** each (no witnesses; no-regression only; the two
       behaviour-visible changes in the batch — getFocusables now
       non-empty, hover chain — regressed nothing). Seven items stale
       with reasoning (149 AOSP base View returns false and ATL has no
       ViewGroup interception machinery; 152 implemented, the HACK is
       the fork's deliberate Open Sudoku catch; 153 implemented, TODO is
       only the toplevel edge case; 155 AOSP base computeScroll IS
       empty; 159 no ActionMode infrastructure, null faithful; 161 no
       activity-manager telemetry sink). Two queue items this batch were
       **atl-gaps false positives**, worth a tooling fix: 154
       (`scrollTo` "is marked TODO") matched the prose *"AOSP does
       nothing"* inside an explanatory comment, and last batch's 143
       (`resolveAttribute` "prints on every call") was already
       debug-gated — the `TODO_MARK` regex (`scripts/atl-gaps:69`) needs
       to ignore prose in comments and print sites.
     - Queue batch 164–178 (fork `b7e73113`..`433b2354`): isShown now
       AOSP's real walk (this + ancestors VISIBLE and attached — ATL's
       chain ends at `WindowManagerImpl.WindowViewParent`, the role
       AOSP's ViewRootImpl plays, so the AOSP loop works verbatim and
       detached views answer false); getFocusedRect fills the drawing
       rect (empty body → callers got a 0x0 rect);
       transformMatrixToGlobal/transformMatrixToLocal walk the parent
       chain over real left/top/scrollX/scrollY (were no-ops, so popup
       and drag-shadow math saw identity; the Local half was absent);
       hasOnClickListeners mirrors the stored listener (said false even
       right after setOnClickListener); tooltip text state + the
       previously absent getTooltipText; Activity.getWindowManager keeps
       one WindowManager per activity (built a fresh one per call while
       AOSP keeps one from attach()); Intent ClipData state. Gate
       `fc47` vs `fc46`: **0 improved, 0 regressed** (no witnesses; the
       three behaviour-visible changes regressed nothing). The rest were
       recorded after checking AOSP, with the reason written into the
       code where the constant is faithful: requestRectangleOnScreen
       (**gap: scroll-into-view** — ViewParent has no
       requestChildRectangleOnScreen), playSoundEffect (no ViewRootImpl
       callbacks, no sound-effect sink), isHardwareAccelerated (false is
       faithful: no HWUI, no layer/overlay APIs), Activity.onCreateView
       (no framework FragmentManager to delegate to; androidx overrides
       it itself), and the **keyboard-activation gap** (View/Activity
       onKeyDown/onKeyUp): AOSP's confirm-key press+click needs the
       ACTION_UP half, which the fork's View.dispatchKeyEvent drops, and
       checkForLongClick machinery — a partial copy would leave views
       stuck pressed; ATL's back action also comes from JNI straight to
       onBackPressed, so AOSP's BACK branch would double-handle back.
     - Queue batch 179–193 (fork `3cfaf852`..`337d8a92`):
       `OnFocusChangeListener`'s interface body was another `// TODO`
       with **no method at all** — AOSP's `void onFocusChange(View,
       boolean)` added and the listener stored (`getOnFocusChangeListener`
       answers it), but honest gap recorded: nothing calls it, ATL's focus
       is GTK's and the fork has neither `View.onFocusChanged` nor a JNI
       focus-change callback. `setOnKeyListener` stored nothing; now the
       listener is stored and `View.dispatchKeyEvent` asks it first like
       AOSP's `dispatchKeyEventNoClient` (behaviour-visible on the GTK key
       path). `setTranslationZ`/`getTranslationZ`/`setZ` were state-less
       while `getZ()` returned elevation alone — **not** just HWUI
       bookkeeping here, because `ViewGroup.addView` sorts children by
       `getZ()` before handing the index to GTK (`ViewGroup.java:112`), so
       the sort now sees elevation + translationZ as AOSP computes it.
       `isInLayout()` (`return false; // FIXME`) now returns the
       `in_layout` flag the fork already maintains around
       `layoutInternal()`. Foreground drawable state (+`getForeground`),
       `setDefaultFocusHighlightEnabled` (+ the previously undeclared
       getter, AOSP default true), AOSP's reference-counted
       `setHasTransientState`/`hasTransientState` (clamped at 0; parent
       propagation + recycling gate recorded as absent), scroll-indicator
       state + the missing `SCROLL_INDICATOR_*` constants (masked update)
       and live-region state + `ACCESSIBILITY_LIVE_REGION_*`. Four items
       recorded instead of changed, reason in the code: **gap: view
       saved-state pipeline** (`saveHierarchyState`/`restoreHierarchyState`
       are empty because the whole feature is absent — no caller for
       `Activity.onSaveInstanceState(Bundle)`, no id-keyed
       `dispatchSaveInstanceState`, the widget `SavedState` classes are
       commented out in `ExpandableListView.java`), no drawable
       state-animator machinery (180), no accessibility node pipeline
       (188), and `setSaveEnabled` now at least stores its flag. Two more
       stale from AOSP: 186 already debug-gated, 189 `onOptionsItemSelected`
       is just `mParent` delegation in AOSP and ATL has no embedding
       parent.
     - **Gate `fc48` FAILED on the canary (`me.zhanghai.android.files`
       ALIVE→CRASH) and it was a flake, not a regression.** Measured, not
       assumed: the same NPE (`ap3.c → yo3.k → lk.f → ud.r0 …`,
       `Object.getClass()` on null, inside a `Dispatchers.Main.immediate`
       coroutine) reproduces at ~30 % on *both* trees — 2/5 forced reruns
       at the previous tip `433b2354`, 3/8 at the new tip — and a full
       re-run at the new tip (`fc49`) is **0 improved, 0 regressed vs
       `fc47`** (`fc48` vs `fc49` shows the app back to ALIVE). Batch kept.
       The intermittent crash itself is unfixed and worth a look: it is
       app-side code reacting to a null root in its storage listing, i.e.
       the same neighbourhood as the missing SAF document-provider
       subsystem (HANDOFF item 13 family).
     - Queue batch 194–208 (fork `9a1af897`..`5c42fdad`, gate `fc50` =
       **0 improved, 0 regressed vs `fc49`**): a batch of View state that
       was simply dropped — `setScrollContainer`, `setSaveFromParentEnabled`,
       `setAccessibilityHeading`, `setLabelFor`, `setTransitionName` (and
       the AOSP getters `isScrollContainer`/`isSaveFromParentEnabled`/
       `isAccessibilityHeading`/`getLabelFor`/`getTransitionName` were not
       declared at all), plus `getOverlay()` building a **new empty
       `ViewOverlay` per call** (so `add()`/`clear()` were unobservable;
       now one lazy object per view, as `ViewGroup.getOverlay()` already
       did). `clearAnimation`/`setAnimation`/`getAnimation` had no
       `mCurrentAnimation` slot — now kept, with the honest note that ATL
       has no per-frame animation driver (`startAnimation()` runs the
       animation synchronously, an existing gap). **`bringToFront()` was
       empty and `ViewGroup.bringChildToFront` a `// TODO` no-op**: ATL's
       drawing order *is* the native GTK child order (the wrapper
       snapshots its child, GTK paints siblings in order — the same
       reason `addViewInternal()` z-sorts before `native_addView`), so the
       fix reorders the Java list *and* the native widget
       (`native_addView` index = `children.size()` appends, one more than
       the fresh-add case because the child is already attached).
       Five items recorded instead of changed, each with the AOSP check in
       the code: `onHoverEvent` (a11y-hover only, no pipeline), the
       **ComponentCallbacks gap** (the fork's interface is literally empty,
       nothing holds a registration list, ATL has no reconfiguration
       source), `Resources.newTheme` (stale — AOSP returns a fresh Theme
       too and the fork's `Theme()` allocates a real native theme),
       `awakenScrollBars` (no scrollbars in ATL; false keeps the fork's
       own `AbsListView.trackMotionScroll` repaint) and
       `requestFocusFromTouch`'s TODO (resolved: `isInTouchMode()` is
       false because focus is GTK's — the branch AOSP takes then).
       `Activity.requestWindowFeature` now delegates to
       `Window.requestFeature` where the reason belongs (answer unchanged).
     - Queue batch 209–223 (fork `4e59b7c6`..`7a869e95`, gate `fc51` =
       **0 improved, 0 regressed vs `fc50`**): more dropped state fixed —
       `setTextAlignment` (value lost; getter answered 0, default now
       AOSP's `TEXT_ALIGNMENT_GRAVITY`), `setHorizontal/VerticalScrollBarEnabled`
       (+ the two `is…ScrollBarEnabled()` getters, absent; default false
       as AOSP), and `setKeyboardNavigationCluster`, whose getter was
       already declared **and hard-wired to false** — the pair could
       never round-trip. `Configuration.getLocales` returned the *system*
       list while `setLocales` was a commented-out body whose `mLocaleList`
       field was never declared; now the AOSP shape with the lazy
       `fixUpLocaleList()` sync. `getWindowId()` now answers a
       process-wide `WindowId` under the same attach test as
       `getWindowToken`. **Item 209 turned out fixable after all**: my
       first read said `parentActivityName` was never parsed — wrong,
       `PackageParser` fills it fully qualified (`buildClassName`) and
       `PackageManager.getActivityInfo` serves it; the Activity just has
       no `mActivityInfo`, so `getParentActivityIntent` now asks the PM
       (AOSP shape, incl. `Intent.makeMainActivity`, which the fork
       lacked). No corpus witness for it — an up-navigation interaction
       would be needed. Six items recorded faithful (`Theme.rebase`,
       `onSearchRequested` + **gap: search subsystem**, `closeOptionsMenu`
       — AOSP's own gate can never pass in ATL, `applyOverrideConfiguration`
       and `isChild` — embedded-activity facilities ATL lacks,
       `overridePendingTransition` — no transition machinery) plus
       `setOnDragListener` stored with **gap: drag & drop subsystem** and
       `setCameraDistance` (no RenderNode, 3-D properties unconsumed);
       `onCreatePanelView` was stale — AOSP's default returns null.
     - Queue batch 224–238 (fork `d3ada17b`..`e0f09030`): `findFocus()`
       returned `this` **unconditionally** and `getCurrentFocus()` was a
       hardcoded null — now AOSP's shape walking per-view GTK focus
       (`isFocused`, the item-114 machinery), `ViewGroup` recursing,
       `Activity` walking `root_view`. `getLocalVisibleRect` (always
       "nothing visible") now answers via the working
       `native_getGlobalVisibleRect` (local bounds; ancestor-clipping
       gap shared with that method). `shouldShowRequestPermissionRationale`
       returned **true** — AOSP answers true only after a user *denial*,
       which ATL can never produce (no runtime permission dialog) — so
       permission-helper apps were stuck in their rationale-then-retry
       loop; now false. `isShowingLayoutBounds` returned true while no
       overlay is ever painted (AOSP default false). Stored+round-trip:
       force-dark-allowed, padding-relative flag (the getter was
       hardcoded false), `willNotDraw()` (the *setter* already stored!),
       `StateListAnimator` (dropped right after inflation),
       `ViewTranslationCallback`. Recorded: **gap: ServiceConnection
       tracking** (`bindService` really binds + `onServiceConnected`,
       nothing maps connections to services, so unbind cannot
       disconnect), `announceForAccessibility` (a11y pipeline gap);
       stale: `onCreateOptionsMenu` (AOSP default returns true) and
       `loadDrawable` "prints on every call" (all prints gated by
       false constants or commented out; the one ungated `Log.w` is
       AOSP's error path).
       Gate `fc52` **FAILED on the canary again** — measured again:
       forced single-app runs gave **5/10 CRASH at the new tip
       (`e0f09030`) vs 6/10 at the old tip (`7a869e95`)** with the same
       NPE signature (`ap3.c → yo3.k → lk.f → ud.r0`,
       `Dispatchers.Main.immediate`) — the flake rate is *unchanged*,
       and a full re-run at the new tip (`fc53`) is **0 improved, 0
       regressed vs `fc51`**. Batch kept.
     - Queue batch 239–253 (fork `fa6b380b`..`9315ed5e`, gate `fc54` =
       **0 improved, 0 regressed vs `fc53`**): the context-menu listener
       is kept now — `setOnCreateContextMenuListener` dropped both halves
       of AOSP's setter (the store *and* the long-clickable side, which is
       how Android opens the menu), the interface's method was commented
       out so nothing could type an implementation, and
       `registerForContextMenu` was empty though AOSP's body is the
       one-liner `view.setOnCreateContextMenuListener(this)`; Activity now
       implements it with AOSP's empty default. **Gap: context-menu panel
       machinery** (no showContextMenu/createContextMenu, no panel
       manager) stays recorded — the listener is stored, never called.
       `getCallingPackage` was hardcoded null while the fork *already*
       records the caller (`resultActivity`, used by
       `getCallingActivity`/`setResult`) — now that caller's package,
       still null unless started for result, as documented. TouchDelegate
       round-trips now (was store-nothing + a getter hardcoded null;
       **gap: touch delegation in ATL's input path**), and the four
       `setNextFocus…Id` setters store with AOSP's `NO_ID` default plus
       the AOSP public getters the fork never declared (**gap: Android
       focus-search subsystem** — focus is GTK's). `getCallingPackage`,
       next-focus, `setEnter/ExitSharedElementCallback` (stored, AOSP's
       body is a store) recorded with the **activity-transition gap**
       alongside `postpone`/`startPostponedEnterTransition`, and
       `isAccessibilityFocused` faithful-false (no a11y node pipeline;
       GTK's focus is not Android's accessibility focus). Four were
       **stale**: `Drawable.jumpToCurrentState` (AOSP's base body is
       literally empty; the fork overrides it where it animates),
       `onPrepareOptionsMenu` (AOSP default returns true),
       `isInPictureInPictureMode` (fork already comments why false),
       `isLayoutDirectionResolved` (the fork resolves direction at
       inflation and answers LTR). No regression from the new
       long-clickable side.
     - Queue batch 254–268 (fork `00f9cac2`..`33f1fedd`, gate `fc55` =
       **0 improved, 0 regressed vs `fc54`**): real behaviour fixes this
       time. `isDirty()` always said false — ATL *does* run Android's draw
       path for Java views (GTK's draw calls `View.draw` → `onDraw`, see
       `WrapperWidget.c`), so both moments exist: the flag is raised in
       `propagateInvalidation` (the fork's single invalidation funnel) and
       cleared in `draw()`. `dispatchSetPressed` was empty while AOSP's own
       body re-applies the flag **and invalidates** — a pressed state
       changed Android-side state without ever scheduling the repaint
       (the fork's ViewGroup propagation was already AOSP's shape).
       `performHapticFeedback` now returns false when
       `setHapticFeedbackEnabled(false)` was called (observable: ATL's
       vibrator is real, item 136's work). `stopService` refused any intent
       without an explicit component while `startOrBindService` resolved
       action-only intents through the manifest — both now share one
       `resolveServiceComponent`, so a service started by action can be
       stopped by action. Also round-tripping: `Intent` source bounds (AOSP
       copies in and out), `ShortcutIconResource.fromContext` (the class
       had **no fields**, so a shortcut icon pointed at nothing — now
       packageName + `pkg:type/entry`), `Activity/Window` volume-control
       stream (AOSP routes it through the window; **gap: no Android-side
       volume-key routing**), `isDrawingCacheEnabled` (**gap:
       drawing-cache/snapshot subsystem** — AOSP renders with its own
       software renderer, ATL draws via GTK). `getReferrer` follows AOSP's
       chain (extra → extra-name → `android-app://<calling package>` using
       the caller recorded in the previous batch's `getCallingPackage`).
       Recorded faithful: `getMovie` (AOSP's `Movie.decodeStream` returns
       **null** for non-GIF data, so null is AOSP's own answer; **gap:
       Movie/GIF decoding**), `setTaskDescription` (no task/recents host),
       `getAttributionTag` (AOSP's Context default is literally null).
       `grantUriPermission`/`revokeUriPermission` printed on every call and
       did nothing: **gap: URI permission enforcement** (one app per
       process, no provider access checks to grant into) — prints removed.
       Stale: `Context.getThemeResId` (abstract declaration; `ContextImpl`
       returns `mThemeResource`, which its ctor/`setTheme` set) and
       `Context.stopService` (abstract; the real gap was in
       `ATLLoadedApp`, now fixed).
     - Queue batch 269–283 (fork `3a3510b7`..`7f28b7aa`): `sendOrderedBroadcast`
       printed and delivered *nothing* — now it goes out through the working
       `sendBroadcast` path plus AOSP's guaranteed final `resultReceiver`
       delivery (**gap: ordered broadcast plumbing** — ordering, result
       plumbing, permission check, handler scheduler stay unimplemented).
       `Activity.showDialog(id, args)` returned false doing nothing although
       the managed-dialog map works — now AOSP's flow with AOSP's args hook
       pair, and `showDialog(id)` delegates as AOSP's does. `filterHashCode`
       was a constant 0 — now AOSP's sum. The system-UI visibility listener
       is stored (its interface method had been a TODO stub; no system bars
       can fire it), `requestFitSystemWindows` routes to the fork's real
       `requestApplyInsets`, fading-edge flags and the scrollbar style are
       stored with AOSP's masks/defaults (**gap: edge-effect/scrollbar
       drawing in ATL**). `moveTaskToBack` returned `true` having moved
       nothing — now AOSP's cannot-move answer `false` with the **activity
       task stack gap** named; `onNavigateUp` now makes AOSP's null-parent
       gate from the manifest instead of being a constant before looking.
       Stale/faithful: `createPackageContext` (abstract; `ContextImpl`
       implements), `NotFoundException()` (AOSP's ctor is literally empty),
       `getWallpaper` (null is AOSP's own no-wallpaper answer),
       `onKeyLongPress` (AOSP default literally false), drawing-cache
       siblings. Gate `fc56` **FAILED on the canary**, then `fc57`/`fc58`
       also showed it — measured, not assumed: 6 forced single-app runs on
       each tree gave **1/6 CRASH at the new tip vs 5/6 at `33f1fedd`**, and
       the logs show *two* pre-existing flake modes at both tips — the known
       NPE (`ap3.c → yo3.k → lk.f`) and a JNI abort in the app's own
       syscall lib (`Syscall.getgrgid` pending-AIOOBE + `NewByteArray`) which
       hit the old tree twice (`f56or2/3`) and is what `fc58` showed. Old
       tree strictly worse on identical signatures: batch kept.
     - Queue batch 284–298 (fork `0183c9f4`..`7db65a56`):
       `setOnGenericMotionListener` was doubly dead — the interface had no
       method (TODO stub) and the setter dropped the listener — while ATL's
       GTK scroll controller feeds real `ACTION_SCROLL` events through
       `dispatchGenericMotionEvent`, so storing the listener and using
       AOSP's listener-then-override dispatch order is observable.
       Scrollbar fading/position and accessibility traversal ids are now
       stored (gaps: scrollbar drawing, a11y node pipeline; AOSP's
       `SCROLLBAR_POSITION_*` constants added — verified there are no
       START/END variants). `setFinishOnTouchOutside` now routes to the
       window (AOSP's `Window.setCloseOnTouchOutside`; gap: no
       outside-touch dispatch), and `setDefaultKeyMode` stores the mode
       **on the activity** where AOSP keeps it (verified against
       android16-release, not on Window) with the five `DEFAULT_KEYS_*`
       constants; the unhandled-key fallback needs the search /
       menu-shortcut / dialer subsystems ATL lacks. Stale/faithful:
       `isDeviceProtectedStorage` (ContextImpl reads `atl_flags`),
       `revokeUriPermission` (matches the recorded non-enforced grants),
       `getPairValue` (old-AOSP `@hide` body verbatim, `Log.w` included),
       `onKeyMultiple` (AOSP default literally false), `native_drawChildren`
       (ViewGroup re-declares it native with its C impl; View-side empty
       body correct), `dispatchSetSelected/Activated` (AOSP base bodies
       literally empty), progress-bar and preview-screenshot setters (AOSP
       routes them through `hasFeature`, always false in ATL, and recents,
       which ATL has no host for). Gate `fc59` vs `fc58`: 0/0; the
       canary's JNI flake mode appeared in `fc58`+`fc59` — same `getgrgid`
       signature as the old tree's forced runs `f56or2/3`.
     - Canary JNI flake mode **root-caused and fixed in the harness**
       (D18): Android has no `/etc/group` so bionic's `getgrgid` always
       answers "not found" and MaterialFiles' null-handling runs; under
       bwrap the app saw the host's group database through glibc NSS,
       took the success path no device takes, and its own JNI aborted
       the process. `atl-sandbox` now binds `/dev/null` over
       `/etc/group`: forced canary runs went JNI-mode 2/6 → 0/6
       (`f60nb1..6`, remaining crashes are the separate NPE mode), and
       gate `fc60` vs `fc59` shows **canary CRASH→ALIVE, 0 regressions**.
       Masking `/etc/passwd` too was tried and reverted on the spot:
       the art_standalone libcore copy asserts in `System.<clinit>` on
       a failed `getpwuid` (AOSP main falls back to `user.name=
       "unknown"`); every app died at startup (`f60nr1..6`). Follow-up:
       align art_standalone's `System.java` with AOSP, then mask
       passwd too.
     - Queue batch 299–313 (fork `0b6fd2d9`..`bb9a2c73`):
       `finishActivity(requestCode)` was a logged stub with a "track
       started activities" TODO — ATL creates the for-result children
       itself (`startActivityForResult` records `resultRequestCode`/
       `resultActivity` on them), so the request-code→child map now
       lives on the caller where AOSP's activity manager keeps it:
       finishActivity finishes the right child and delivers
       RESULT_CANCELED unless it already reported; `setResult` gained
       AOSP's dispatch-exactly-once guard. Unknown codes stay a silent
       no-op as in AOSP. Dropped the per-intent startup dump in
       `createMainActivity` (failure prints before `System.exit(1)`
       kept). Stale/faithful: `getVerticalScrollbarWidth` (0 = AOSP's
       answer with no scroll cache; scrollbar drawing gap),
       `isParcelled` (no `mParcelledData` representation in this fork
       at all), Resources `verifyPreloadConfig`/`load*Color*` (AOSP
       bodies verbatim — every flagged print is gated by
       `TRACE_FOR_PRELOAD`/`TRACE_FOR_MISS_PRELOAD`/`DEBUG_LOAD`, all
       false), `layoutInternal` (flag matched the prose "no-op" in a
       comment — TODO_MARK false positive, follow-up already owed),
       `isScreenRound` (intentional `ATL_IS_WATCH` hook), `forPair`
       (old-AOSP `@hide` body incl. AOSP's own TODO),
       `get_atl_loaded_app` (abstract on Context, ContextImpl
       implements), `Menu.close`/`MenuItem.setChecked`/`setTitle`
       (interface declarations flagged; PopupMenu's impls have real
       bodies — close() dismisses the popup). Gate `fc61` vs `fc60`
       **FAILED on the canary**, measured not assumed: 10 forced runs
       at the new tip (7/10 CRASH, **all the same pre-existing NPE
       mode** `ap3.c → yo3.k`, JNI=0 — group mask holding) vs 3/6 at
       the old tip (`f60nb1..6`); the canary logs 0
       `startActivityForResult` hits, so the batch's code paths are
       unreachable in it — Fisher-exact p≈0.6: flake, batch kept.
     - Queue batch 314–343 (fork `cd5e7276`..`db7efb0e`): 30 of the
       30 items were the scanner flagging **interface declarations**
       in `Menu.java`/`MenuItem.java` as empty; `PopupMenu`'s
       `MenuImpl`/`MenuItemImpl` have real bodies for all of them
       (add/clear/removeItem manage the GMenu rows, size/findItem/
       getItem work, state setters store and their getters read).
       Genuine gaps found in that review: `setActionView(View/int)`
       discarded and `getActionView()` answered null where AOSP's
       `MenuItemImpl` (real path verified:
       `core/java/com/android/internal/view/menu/MenuItemImpl.java`,
       android16-release) stores the view — gave it the item id when
       it had none — and inflates for the resId overload; now stored/
       inflated AOSP-style (hosting gap: a GMenu row can't display
       the view). Recorded faithful: `expandActionView`/
       `isActionViewExpanded` false = AOSP's own popup answer
       (expansion needs a presenter that can host it), 
       `setIconTintList` (action-bar-only, no getter at this API).
       Gate `fc62` vs `fc61`: 0/0 (canary ALIVE in both).
     - Queue batch 344–358 (fork `84a4aa48`): more interface-declaration
       false positives (addSubMenu/removeGroup/setGroupVisible/
       hasVisibleItems/getOrder/hasSubMenu are real in `PopupMenu`'s
       impls; expandActionView/setOnActionExpandListener/
       setShowAsActionFlags answers match AOSP's popup semantics).
       One genuine gap: the fork's `MenuItem` **never declared AOSP's
       `getAlphabeticShortcut()`/`getNumericShortcut()`** and the
       shortcut setters dropped their chars — apps reading a shortcut
       back hit NoSuchMethodError. Getters added (verified against
       android16-release `MenuItem` + the internal `MenuItemImpl`);
       setters now store AOSP-style (alphabetic lower-cased,
       unchanged = no-op; setShortcut stores both). Gate `fc63` vs
       `fc62`: 0/0.
     - Queue batch 359–373 (fork `66f2e96c`..`117b8d03`): 13 of 15 stale
       again — Menu/MenuItem/**AttributeSet interface declarations**
       (359, 360, 367–373; `XmlPullAttributes` and
       `DecompiledXmlResourceParser` carry the real bodies), and
       AOSP-verbatim code the scanner mis-flags (`registerMenu` ×2 —
       android16-release `MenuInflater` declares both empty layoutlib
       hooks with that same comment and calls them at the same two
       sites; the private `MotionEvent()` pool ctor; `MenuState.setItem`'s
       only print is AOSP's own `Log.w` on the double-action-view error
       path, never on the normal path). One real fix:
       `MotionEvent.toString()` printed the literal **"FIXME"** for
       toolType/buttonState/metaState/flags/edgeFlags/historySize/
       downTime — every getter and `*ToString` helper already existed in
       the file, only commented out; now AOSP's real calls (verified
       against android16-release `MotionEvent`, older field list kept:
       no actionButton/classification APIs here). Recorded faithful:
       `addIntentOptions` answers 0 (AOSP adds one item per
       `queryIntentActivityOptions` result, and ATL's hook returns null
       → AOSP's own code gives N=0). Gate `fc64` vs `fc63`: **1
       improved** (canary `me.zhanghai.android.files` CRASH→ALIVE — its
       NPE flake mode, not caused by this batch, which only changed
       `toString`), 0 regressed. `117b8d03` (comment-only, diff
       verified) is installed on top of the tree `fc64` ran.
     - Queue batch 374–388 (fork `5321379d`..`0a9f833e`): the
       **MotionEvent state cluster** — the whole `obtain(...)` family
       took `downTime`/`metaState`/`buttonState`/`deviceId`/`edgeFlags`/
       `flags`/`pressure`/`size` and threw every argument away (the
       `core/jni/android_view_MotionEvent.cpp` natives that would have
       stored them are not part of this fork — only the generated
       header exists), so every getter answered a constant even for
       events the caller described in full, and `getDownTime()` returned
       `eventTime` (gesture duration always 0). Now stored in Java
       fields (this class already keeps source/action/eventTime/ids/
       coords in Java; the JNI only calls the constructors): the
       constructors remember the gesture's ACTION_DOWN time the way
       AOSP's input layer does, the getters read the state, and the
       defaults for produced events are AOSP's own values for this
       device shape — every ATL event comes from one virtual
       `SOURCE_TOUCHSCREEN` with no pressure/size axes, and
       frameworks/native `TouchInputMapper` (android16-release) gives
       such a device `pressure = isHovering ? 0 : 1` (default
       PressureCalibration) and size 0 (`SizeCalibration::NONE`), so
       pressure is 1.0 while touching (was 0) and `getSize(int)` lost
       its arbitrary 1. `getDeviceId()` now answers 0, the single id
       `InputDevice.getDeviceIds()` lists (was an arbitrary 1 +
       FIXME); `findPointerIndex` searches ids and answers -1 (it said
       0 for every id); the single-pointer `obtain` used pointer id 1
       (AOSP: 0) and raw coords 0 (AOSP: raw == x/y). Also fixed a
       live-crash class the queue never flagged: `scale()`,
       `isTouchEvent()`, `setTainted()`, `setDownTime()` still called
       the never-provided natives → **UnsatisfiedLinkError** on any
       call (`isTouchEvent` is public since API 29); implemented in
       Java (hover/scroll rule from the AOSP javadoc) + added
       `InputDevice.SOURCE_CLASS_MASK`. Recorded faithful:
       `getHistorySize()`/`getOrientation()` 0 (no input-reader
       batching / no tilt axis — AOSP's own answers). 5 items stale
       again (`AttributeSet` interface declarations —
       `XmlPullAttributes` carries the real bodies). Follow-ups in
       code: feed GDK modifier state through the JNI input path so
       produced events report CTRL/SHIFT in `getMetaState()`;
       `getAxisValue()` still answers 0 for all axes. Gate `fc65` vs
       `fc64`: 0/0 — **no runtime witness** (atl-gaps: no coverage for
       these gaps; no `obtain`/pressure/UnsatisfiedLinkError
       signatures in corpus logs), so this batch is verified
       no-regression + compile, and the pressure-default change is
       taken from AOSP's input layer, not measured on an app.
     - Queue batch 389–403 (fork `6be26e6c`..`1eb7fbe2`): 11 more
       `AttributeSet` interface-declaration false positives, and
       `PointerCoords()` — AOSP's ctor is empty verbatim (all axes
       default to 0). Three real fixes, all finished work from the
       previous batch's input path: (1) every axis getter
       (`getTouchMajor`/`getTouchMinor`/`getToolMajor`/`getToolMinor`/
       `getOrientation`/`getAxisValue`, and now pressure/size too)
       went through the constant-0 `nativeGetAxisValue` stub and
       discarded the `obtain(...)` caller's `PointerCoords` — the
       samples are now copied per event (AOSP keeps them in the event;
       libinput `PointerCoords::getAxisValue` answers 0 for unset
       axes, matching the produced events whose touchscreen has no
       touch/tool/tilt axes); `getTouchMajor(int)`'s arbitrary 2.5f
       gone; `scale()` scales the touch/tool axes like AOSP's
       `PointerCoords::scale` (not pressure/size/orientation).
       (2) `transform(Matrix)` called the empty `nativeTransform`
       stub — apps mapping event coords through a matrix got no
       transform; transcribed libinput `MotionEvent::transform`
       (android-7.1.0_r7: perspective divide, window coords move,
       raw preserved, orientation rotated via `transformAngle`) and
       deleted the stub. (3) `getEventTimeNano()` multiplied ms by
       1000 — microseconds, a thousandth of AOSP's ns answer; now
       `* NS_PER_MS`. Gate `fc66` vs `fc65`: 0/0; no corpus witness
       (no coverage in logs), compile + no-regression only.
     - Queue batch 404–418 (fork `b5b5113f`..`958cc8d7`): a small
       real batch inside mostly stale flags. Item 404 was stale — the
       flag came from the phrase "does nothing" in my own transform()
       comment from the previous batch (atl-gaps `TODO_MARK` false
       positive), reworded. `FragmentTransaction.commit()` returned 0,
       which in AOSP is the first back-stack index — this shim has no
       back stack, and AOSP's `BackStackRecord.commitInternal`
       (oreo-release) answers `mIndex = -1` when the transaction was
       not added to one, so it now returns -1. `Dialog.onCreate`
       printed "- onCreate - Dialog!" (AOSP's body is empty verbatim,
       oreo-release `Dialog.java:416`) and `show()`'s println was ATL
       dev noise over an otherwise real body — both removed;
       `onTouchEvent` documented instead of implemented (AOSP's
       outside-touch cancel needs `Window.shouldCloseOnTouch`, which
       this fork does not provide, and its native window never
       dispatches `Window.Callback.onTouchEvent` — touches go to the
       view tree). Recorded faithful: the `Uri` items (abstract
       declarations flagged as "empty"; `private Uri() {}` and
       `Part.isEmpty`/`EmptyPart.isEmpty` are AOSP verbatim, checked
       against android16-release `Uri.java`), the `SpannableStringBuilder`
       items (flagged TODO lines are AOSP's own comments verbatim, and
       the `setSpan` Log.e is AOSP's own zero-length-
       SPAN_EXCLUSIVE_EXCLUSIVE error log, oreo-release line 705),
       `ContentResolver.run()` (the SyncAdapterThread Runnable does
       the real `onPerformSync`, only logging at the end), and
       `Dialog.dismiss()` (real body; the flagged HACK comment
       documents the intentional 10 ms delay that fixes NewPipe's
       subscribe-then-dismiss race). Gate `fc67` vs `fc66`: 0/0; no
       corpus witness for commit()/onTouchEvent (no coverage),
       compile + no-regression only; dialogs do open in the corpus,
       so the println removals are gate-covered.
     - Queue batch 419–433 (fork `df6208da`..`03b8eadc`): mostly stale
       flags plus four real dialog/fragment fixes. Stale: 3 more `Uri`
       abstract declarations, 3 `Spanned` interface declarations, and
       `Uri$Builder.Builder() {}` / `OpaqueUri.getPath()`/`getAuthority()`
       returning null — all AOSP verbatim (android16-release
       `Uri.java`). `executePendingTransactions()` → the flag wanted
       "state", but AOSP's signature is boolean and `false` is its
       answer with an empty queue (this shim applies operations in
       `add()`) — commented. Real: `hide()`/`Dialog$Builder` println
       noise removed; `setOnCancelListener` was an empty method AND
       `DialogInterface.OnCancelListener` was an empty marker with no
       `onCancel` to invoke — interface method added (AOSP shape),
       listener stored and dispatched on `cancel()` (main looper,
       before dismiss' close, never on plain `dismiss()`), compile
       error it exposed fixed; `findFragmentByTag` always null because
       `FragmentTransaction.add(fragment, tag)` threw the tag away —
       `Fragment.mTag` set and scanned newest-first per AOSP
       `FragmentManagerImpl`; and the six `Window.Callback` menu hooks
       on `Dialog` threw `UnsupportedOperationException` — replaced
       with AOSP's Dialog defaults (oreo-release; onMenuItemSelected
       false, onCreatePanelView null, onPreparePanel true, panel menu
       via the now-present public `onCreateOptionsMenu`/
       `onPrepareOptionsMenu` hooks). The menu-hook path is not yet
       wired to dialog windows in this fork, so that part is compile +
       gate verified, not witnessed; `Dialog$Builder` still lacks
       AOSP's `setTitle`/`setMessage`/.../`create()` (flagged in
       fixed.txt for a later batch). Gate `fc68` vs `fc67`: 0/0.
     - Queue batch 434–448 (fork `bd6c1804`..`f52ecb55`, gates `fc69`
       and `fc70` both 0/0): 10 stale flags (`Uri` abstract
       declarations incl. `buildUpon()`, the `Spanned.
       nextSpanTransition` interface declaration, four `OpaqueUri`
       nulls — all AOSP verbatim, android16-release) and five real
       ones: `Bitmap.compress` wrote the literal bytes "fixme
       Bitmap.compress JPEG" into the app's OutputStream for JPEG/WEBP
       — corrupt-file garbage; now false with nothing written
       (Android's encode-failure answer; ATL's Cairo layer has no
       JPEG/WEBP encoder) plus AOSP's NPE/IllegalArgumentException
       pre-checks; `setDensity`/`getDensity` kept no state — density
       stored per AOSP and `getScaledWidth/Height(int)` got AOSP's
       `scaleFromDensity` formula and the `DENSITY_NONE` constant;
       `setCancelable` and `setOnKeyListener` were empty — state now
       stored per AOSP and `DialogInterface.OnKeyListener.onKey`
       declaration added (it was an empty marker), dispatch points
       have no native counterpart in this fork so they are documented,
       not called; `requestWindowFeature` now delegates to
       `Window.requestFeature` like AOSP/Activity (behaviourally
       neutral, the item-208 no).
     - Item 63 (me.zhanghai.android.files NPE) **fixed with witness in
       `fc40` (CRASH→ALIVE)**. Earlier SAF-provider hypothesis was
       wrong: the real root cause is `View.isLaidOut()` returning
       constant true — the app's `doOnLayout`-style androidx helpers
       saw "already laid out" before any layout existed and ran their
       block immediately, where still-null members NPE'd
       (`Object.getClass()` on null, all R8 frames). Bisected by
       single-hunk builds + `rig smoke`: `2a0f01cb` (scale) and
       `c4c26458` (focus) still NPE'd; the isLaidOut hunk alone
       (window-driven layout / explicit `layout()` tracking) removed
       it and reached onResume. The app then hit a swallowed
       `NoSuchMethodException: isSingleLine []` from its RecyclerView
       path → fixed by `5f900c6d` (TextView single-line state; smoke
       exceptions 1→0), gate `fc41` owed. Separate real gap (unfixed,
       context): ATL still implements no SAF document providers, so
       the app's storage-root list content is limited — different
       feature, not this crash.
     - Queue refreshed after fc39: `atl-gaps --emit-queue 15` on
       bazzite against the `9a22a650` tree + fc39 sqlite → queue now
       119 items; the new 15 are View state/graphics gaps with dex
       references but no logged runtime hits (atl-gaps reports
       3302/3303 gaps without runtime coverage). Despite that, the
       batch did witness me.zhanghai.android.files (see above) —
       dex-reference ranking is weak evidence, not "nothing to gain".

Items 5–8 change behaviour on interaction (typing, clicking, pressing,
preference changes), which the harness can't drive yet: gates show no
regressions but no visible startup change. Gates now run with `--jobs 6` on
bazzite (8 jobs + strata ran one app out of memory once).

## Results to know
- Fossify suite (`corpora/fossify.txt`): **13/16 work** on the phone; Phone (telecom), Launcher (app widgets), Camera (no camera) don't. 13 are on the phone's home screen ("… (ATL)"), via `rig desktop-apps`.
- **Fix-coverage corpus (`corpora/fixcov.txt`, 2026-11-22, +19 apps, DB now 71)**: picked because the base corpus never exercises the staged widget/state/animation fixes. On main: 9 CRASH / 9 ALIVE / 1 EXITED; the crashes were all **new** causes (identical on both builds) and are now fixed on `local-fixes` (HANDOFF item 16): 8 of the 9 crashes gone (`tickmate`, `omninotes`, `siggen`, `tuner`, `kdeconnect`, `gramophone` ALIVE; `diary` and `editor` BLANK = empty content, no entries / empty buffer). Left: `me.zhanghai.android.files` CRASH (NPE in obfuscated app code) and `player.phonograph.plus` EXITED (appintro ViewPager count desync), both queued with evidence. Note: gates now cover all 71 apps, and `compare` scores outcome only — signature deltas need the `failures`-table diff (see this session's transcript).
- Messages opens but can't send/receive: no default-SMS role, no telephony backend (owner decision pending).
- microG: template + A/B tooling work (`--data-template`), no app reaches Play Services yet. ATL bug: microG native libs path (`<apk file>_` vs `<package>_`).
- Owner's wishlist app **Flow** (YouTube client): starts, then exits on missing `AppWidgetManager.getInstalledProviders()`.

## Next steps (roadmap has the full queue)
1. **Audit fixes**: `docs/audit/2026-11-21-atl-audit.md` (139 confirmed, 41 high; leniently verified, re-check each). Done: password masking, EditText/Enter, compound button clicks, SharedPreferences listeners, layer-list state, drawRect stroke. Next suggested: gradient shaders (`<shape><gradient>` draws solid black), Bitmap pixel channel swap / `getPixel` exiting the process, TextView `maxLines`/`ellipsize`, dialogs (`setView`, `setCancelable`), locale in AssetManager. (Theme cache key: merged in the overnight batch.) The two loop-rejected retries are done (staged on `local-fixes`): ProgressBar `max/progress` (`c3b2684b`) and DrawableContainer.setBounds (`2ea23604`).
0. **Fossify Music playback** — **done 2026-11-21** (fork main >= `9a2cd6c4`,
brief updated with root cause: `UserHandle.getCallingUserId()` uid mismatch,
see HANDOFF item 13): tapping a track now loads the player ("Alpha Song 1")
and the interaction script passes on mini and in the bazzite gate. Follow-ups
left: ~~AudioTrack `stop()`/`flush()` stubs~~ done in `b858fdff` (HANDOFF
item 25), `getTimestamp` returns false,
`AnimationUtils.loadInterpolator` returning null (crashes Music's progress
indicator), and mini's Lima audio device failing `snd_pcm_hw_params` — root
cause found 2026-11-23: **Lima attaches no audio device to the `atl` guest at
all** (and the cloud image lacked `kernel-modules`, so no `virtio_snd.ko`);
under VZ with virtio-sound the whole chain works (vm-launcher doc §8). Phone
gate with the new main not run yet.
1a. **exe/DMG VM-launcher distribution (owner question, researched 2026-11-23)**: [`docs/research/vm-launcher-distribution.md`](docs/research/vm-launcher-distribution.md) — feasible, far cheaper than the five native walls; one guest image (the bootstrap rootfs + a drop-directory daemon), wrapped as a `.wsl` double-click distro on Windows (WSLg = per-app windows, Start-Menu shortcuts, Pulse audio glue) and a VZ-VM `.app` on macOS (virtio-gpu/sound/fs). macOS evidence steps done 2026-11-23 (mini): **VZ boot PASS** (cloned Lima disk boots headless under vfkit and under our own ~60-line Swift VZ launcher, Fossify Calculator interacts OK) and **audio PASS** (virtio-sound device → PipeWire sink, `aplay` rc=0, Musicplayer's AudioTrack opens clean — see §8 of the doc). vfkit has no sound device: the product launcher needs `VZVirtioSoundDeviceConfiguration` (macOS 15+); spike launcher in `scripts/research/vzsnd.swift`. Remaining step: WSL2 spike (owner todo, no Windows box set up yet), plus listening-by-ear on a Mac with speakers. **Design doc written 2026-11-23 (D22): [`docs/launcher-design.md`](docs/launcher-design.md)** — shape-only per owner (Andrun pre-release; must keep exe/dmg/flatpak/appimage shippable), stage-gated on the macOS display spike + WSL2 spike, nothing to build yet.
1b. **Interaction scripts** (`interactions/`, docs/usage.md): Calculator (7+8=15), Notes (typing) and Fossify Music (library, playback, + a failing `setView` dialog check) are gated — and since P2 (2026-11-23) they address UI **by element** (`wait-find`/`tap $el`) where the app uses classic views, with a distinct `element` failure signature on a miss; Gramophone stays coordinate-based until P3 (Compose). Status and candidates: `docs/agent-tasks/test-first.md` "Status" (maxLines/ellipsize needs an OCR-DSL step first, e.g. `expect-no-text`). Add scripts for more apps, especially ones exercising fixes 5–8 (checkbox clicks, Enter/IME actions, preferences).
1c. **Installer chain (owner-flagged 2026-11-23, backlog)** — installing apps
*from* apps (F-Droid client, Obtainium): first evidence step + API surface in
[`docs/agent-tasks/installer-chain.md`](docs/agent-tasks/installer-chain.md);
witness list `corpora/stores.txt`. Security staging unchanged (roadmap:
sandbox → mediated installs → permission model → isolation); no root `su`
ever, no widening sandboxes to fake it.
1d. **jvmrig finding #1 (fork-fix queue candidate)**: `KeyEvent.changeTimeRepeat`
misses AOSP's `repeat != 0 ⇒ FLAG_LONG_PRESS` rule (the fork's ctor copy skips
the `mFlags` update) — flagged warn-only (`RIG-KNOWN-GAP`) in
`tools/jvmrig/tests/KeyEventMetaTest.java`; fix it in the fork and the test
will print `RIG-GAP-CLOSED`.
2. App widgets (`AppWidgetManager`, `AppWidgetHost`): unblocks Flow and Fossify Launcher.
3. Visual check of password masking on the phone (an app with a password field).
4. Harness: input scripting, stop-early settle (census is 98% fixed 25 s waits). (Per-app sandbox: done, see above.)
5. Owner: run the Flatpak move on the phone (see below), install Spectacle, merge the two fedora-atomic-phone branches if happy.

## Harness media library
`[atl] media_dir = "work/test-media"`: every run sees a fixed library made by
`scripts/runner/make-test-media.sh` (two albums, Opus + FLAC, macOS junk files,
a photo). `interactions/org.fossify.musicplayer.txt` checks albums, tracks and
(failing until playback works) the player.

## Waiting on the owner
- **Rust path: DECIDED 2026-11-23 (D21) — C then D, B conditional.** Owner:
  run option C (JVM unit-test rig) first, then the option D spike (one leaf
  component rewritten Rust+JNI, measure glue cost); option B (incremental
  C→Rust migration of `api-impl-jni`) becomes the long-term goal only based
  on D's result. C is started: `tools/jvmrig` v0 (brief:
  [`docs/agent-tasks/jvm-test-rig.md`](docs/agent-tasks/jvm-test-rig.md)),
  verified on bazzite AND on the workstation (owner installed
  `java-25-openjdk-devel` 2026-11-23; `work/core-all_classes.jar` staged;
  all api-impl files javac on a stock JVM, smoke test PASS both machines).
  Coexistence mechanics (how the C core and a future Rust core
  talk during a transition): `docs/research/rust-rewrite.md` §10.
- **GTK4 / cross-platform toolkit question — answered** in
  [`docs/research/graphics-backend.md`](docs/research/graphics-backend.md)
  (short: toolkit barely affects APK compatibility; the cross-OS walls are
  ART-host/audio/session-stack, not GTK; keep GTK4 as the Linux backend,
  introduce a host seam only if/when B runs; Windows via WSLg needs no
  swap). Owner decision wanted only if cross-OS native builds become a goal.
- Telephony/SMS design for Messages and Phone.
- Merging the two fedora-atomic-phone branches (`plasma-mobile-spectacle`, `oneplus12-flatpak-home`); both are already applied on the phone by hand.
- **Phone music playback check** (roadmap step "phone gate"): largely done —
  the phone is built to `local:cd4d0a78` and the owner has been playing music
  there (Gramophone reports drove HANDOFF items 25–26). Still owed by the
  owner: a re-listen after the `main` merge — does the resume symptom ("play
  after pause does not start again") still happen, and is the FLAC stutter
  still there (our side: queue items 103–104).

## Done by the owner (2026-11-21)
- Spectacle installed on the phone; `scripts/phone-shot` verified (3/3 captures).
- Flatpak data moved to /home with `move-flatpak-to-home.sh`: root 97% → 52% full (3.7 GB free).
- License chosen: atl-harness is AGPL-3.0-or-later (decisions D15, LICENSING.md).

## Per-app sandbox (2026-11-21)
Every app on the phone runs in its own bwrap sandbox (`docs/sandbox.md`,
policies in `~/atl-apps/<pkg>.policy.json`, `ATL_NO_SANDBOX=1` to compare).
Harness: `atlh run --sandbox`. Done 2026-11-21: the full 52-app A/B gate ran
on bazzite (plain `plain1` vs sandboxed `sbx1` of build 9a2cd6c4): 52 apps in
both runs, 0 improved, 0 regressed — evidence in docs/sandbox.md "Rollout".
`sandbox = true` is now the default in config.toml and config.aarch64.toml.

Network policy modes (D16 follow-up): `"none"`, `"internet"` (default with
INTERNET, no LAN), allow-lists for LAN/Tailscale (Jellyfin, Home Assistant),
`"full"`; see docs/sandbox.md "Network". Phone rootfs needs a rebuild
(`rig -r oneplus12 rootfs --from mini`) to get passt/nftables before an app
with INTERNET runs there.

App widgets: staged plan in `docs/agent-tasks/app-widgets.md` (stage 1 =
local model, unblocks Flow; stage 3 = Claude design: cross-sandbox service).

Late 2026-11-21 (Claude): fork main 63bf04e7 adds media playlists, a Typeface
model, RippleDrawable radius (Gramophone gets further), widget stage 1
(Flow starts; renders blank: Compose). Gramophone and Flow are installed on
the phone. Phone audio goes through PipeWire (rootfs has pipewire-alsa; the
sandbox gives no /dev/snd: raw hw: bypasses the speaker protection).
fedora-atomic-phone main has a flash.sh guard (FLASH_PLAN=manual on
oneplus-waffle), docs/agent-tasks/safety-audit.md and port briefs. Research
on other ATL forks: docs/research/atl-forks.md (when committed).

## Runner use (several agents: see AGENTS.md)
Say here which runner you're using for builds/gates, and clear it when done.
- opencode (this session, 2026-11-21 evening): used `bazzite`, `mini` and
  (claimed but not used) `oneplus12`; all released 2026-11-21. bazzite and
  mini have fork main `7cb4cd2b` installed (`rig build --worktree
  ~/Develop/atl-albumart`, tree identical to what was merged).
- **opencode (this session, 2026-11-21 night): `oneplus12`** — ATL build +
  owner music-playback check (task: phone gate). Not shared.
- **opencode (this session, 2026-11-22): `bazzite`** — finished overnight
  task 20 (build + gate `agent-20261121-185746-20b`), reviewed the whole
  `agent/overnight` batch, re-did the two loop rejections (gates `pbmax1`,
  `dc1`); per owner instruction everything is staged on `local-fixes`
  (tip `2ea23604`) instead of `main`, and `main` was rewound to
  `7cb4cd2b`. Left bazzite installed with clean main `7cb4cd2b`
  (nothing agent-branch-specific). Released.
- **opencode (this session, 2026-11-22, later): `bazzite`** — fix-coverage
  corpus (`corpora/fixcov.txt`): fetched 19 apps, ran them against main
  (`fixcov-main`) and `local-fixes` (`fixcov-lf`), fed 4 new queue items from
  the crashes. Left bazzite installed with clean main `7cb4cd2b`. Released.
- **opencode (this session, 2026-11-22, third): `bazzite`** — fixed the fixcov
  crashes on worktree `~/Develop/atl-fixcov`, branch `fix/fixcov-crashes`
  (off `local-fixes`); gates `fcbase`→`fc01`→`fc02`→`fc03`→`fc04`→`fc07`,
  each step 0 regressions (see HANDOFF item 16). `local-fixes` fast-forwarded
  to `44897c0d` and pushed; `main` untouched at `7cb4cd2b` (D17). The
  overnight agent loop was live on bazzite throughout: its `agent-*` runs
  share `work/atlh.sqlite`, so per-app signature *counts* drift between runs,
  and the `fc05` run rows disappeared from the DB (`fc01..fc04` survived) —
  latest usable bazzite baseline `fc04`, newest full run `fc07`. Bazzite is
  left installed with the `local-fixes` tip `44897c0d` (built from
  `atl-fixcov`) because the agent-loop defaults to `local-fixes` — say so
  before building over it. Released.
- **opencode (this session, 2026-11-22, fourth): `mini` (aarch64)** —
  verifying the fixcov batch on arm64: fetched the fixcov APKs into mini's
  corpus (`atlh fetch --pkg-file corpora/fixcov.txt`, DB now 67 apps), ran
  `mfcbase` on mini's previous build (`local:f5753053`), built
  `~/Develop/atl-fixcov` with `rig -r mini build --worktree ...`, then ran
  `mfc01` (8 improved / 0 regressed), `mfc02` (10-app check) and `mfc03`
  (full, 0 improved / 0 regressed vs `mfc01`). That found the aarch64-only
  `HorizontalScrollView.computeScroll` NPE, fixed in `44897c0d`. Mini is left
  installed with `local:44897c0d`. Released.
- **opencode (this session, 2026-11-22, fifth): `oneplus12`** — over the LAN
  (`waffle.local`) because USB networking won't enumerate here. Built
  `~/Develop/atl-fixcov` on the phone and installed `local:44897c0d`, fetched
  the fixcov APKs into its corpus, ran `pfc01` (sandbox on, unusable there),
  `pfc02` (`--no-sandbox`) and `pfc03` (after adding `config.phone.toml`,
  `sandbox = false`, selected for that runner in `runners.toml`; `pfc02 ->
  pfc03` 0 improved / 0 regressed). Nothing outside `/home/phone`, no
  kernel/system changes. Phone left installed with `local:44897c0d`.
  Released.
- **opencode (this session, 2026-11-22, sixth): `bazzite`** — built
  `~/Develop/atl-fixcov` with the four queue fixes (queue items 21–24) and
  ran the gate `fc08` (71 apps, 0 improved / 0 regressed vs `fc07`).
  `local-fixes` tip now `7f906b29`, pushed; `main` untouched (D17). Bazzite
  left installed with the `local-fixes` tip `7f906b29` (dirty at build time,
  commits came after). Released.
- **opencode (this session, 2026-11-22, seventh): `bazzite`** — gates
  `fc09`–`fc12`, the verify-batch fetch/A-B (`vbase`/`vtip`) and the
  queue fixes through `c75de5c0`. Bazzite left installed with the
  `local-fixes` tip `c75de5c0` (dirty at build time, commits came
  after). Released.
- **opencode (this session, 2026-11-22, eighth): `bazzite`** — gate
  `fc13` (davdroid CRASH -> ALIVE, 0 regressed) and the fixes through
  `3a438448` (ConnectivityManager, PopupMenu, DocumentsProvider port).
  Bazzite left installed with `local:c75de5c0-dirty`, which is the
  `local-fixes` tip `3a438448` tree. Released.
- **opencode (this session, 2026-11-22, ninth): `bazzite`** — gate
  `fc14` and the Vibrator/ContentObserver fixes through `755e604d`.
  Bazzite left installed with `local:3a438448-dirty`, which is the
  `local-fixes` tip `755e604d` tree. Released.
- **opencode (this session, 2026-11-22, tenth): `bazzite`** — gate
  `fc15` and the ViewRootImpl/getViewRootImpl fix `083e0c4e`. Bazzite
  left installed with `local:755e604d-dirty`, which is the `local-fixes`
  tip `083e0c4e` tree. Released.
- **opencode (this session, 2026-11-22, eleventh): `bazzite`** — gates
  `fc16`–`fc18` (lightnovelreader CRASH -> ALIVE) and the queue-tail +
  interpolator fixes through `7ccb4a77`. Bazzite left installed with
  `local:ed3f2760-dirty`, which is the `local-fixes` tip `7ccb4a77`
  tree. Released.
- **Phone state, 2026-11-22 18:53:** user ran
  `rig -r oneplus12 build --worktree ~/Develop/atl-fixcov --detach`;
  build OK (17s incremental), phone runner now installed with
  `local:7ccb4a77` = `local-fixes` tip (was `local:44897c0d`). No
  reboot or restart needed — each app run spawns a fresh ATL process.
  Phone runs still need `--no-sandbox`. (Update: `local-fixes` advanced
  to `a0ac74ca` right after — the phone is two commits behind; rerunning
  the same build command brings it current.)
- **opencode (this session, 2026-11-22, twelfth): `bazzite` + `oneplus12`** —
  speedups (bazzite `jobs` 6 validated by gate `fc21`, ccache installed on
  all three runners incl. the phone rootfs via file copy from mini;
  bazzite builddir re-set up with the ccache launcher, clean rebuild
  17s→7s), then the Gramophone chain (HANDOFF item 25): gates `fc22`
  (sendSessionEvent) and `fc23` (Rating), both 0 regressions; scratch
  single-app runs `gpre*`/`gpost*` under `work/config-mp.toml`. Phone
  rebuilt twice by the user (`22111b88`, then `22111b88-dirty` = tip
  `465bb53b` tree, 19:58). Both runners released; bazzite installed with
  the `465bb53b` tree (= `local-fixes` tip).
- **opencode (this session, 2026-11-22, thirteenth): `bazzite`** — finished
  the Gramophone audio chain (HANDOFF item 25): scratch runs `gpost1`–
  `gpost12` under `work/config-mp.toml` (a temporary `~/.asoundrc` null ALSA
  device gave reproducible timing for the stalled-write repro; it was removed
  before the gates so the gate environment matches the `fc23` baseline — no
  asoundrc). Fork commits `b858fdff` (AudioTrack) + `9690cf54` (FLAC
  decoder); gates `fc24` (19 fixcov apps, and it exercised the extended
  gramophone interaction script) and full-corpus `fc25` (74 apps) vs `fc23`:
  0 regressions both. `local-fixes` pushed through `9690cf54`. Phone rebuild
  to that tip started (`rig -r oneplus12 build --worktree ~/Develop/atl-fixcov
  --detach`); owner re-test of skip/seek pending. Bazzite left installed with
  a clean `local:9690cf54` (= `local-fixes` tip). Released.
- **opencode (this session, 2026-11-22, fourteenth): `bazzite` +
  `oneplus12`** — Gramophone audio follow-ups (HANDOFF item 26). Phone:
  builds of the `atl-fixcov` worktree (`build-atl`) and the audio repros —
  scripted play/pause/resume runs `pres1`–`pres3` (the SIGSEGV witness pair)
  and the FLAC-stutter repro runs `coal1`–`coal4` under
  `work/config-coal.toml` (real Taishi album in `work/music-taishi`, tap on
  the Coalescence play icon); temporary ALSA-state and write-path probes were
  added and removed in the same worktree, never committed. Bazzite:
  `build-atl` plus full-corpus gate `fc26` (74 apps) vs `fc25`. Fork commits
  `9e4abf80` (failed-open SIGSEGV) and `cd4d0a78` (PathMeasure/
  PathInterpolator/AccelerateDecelerateInterpolator) on `local-fixes`, both
  runners left installed at a clean `local:cd4d0a78`. Checked `rig job status`
  on both before starting; no other agent's jobs were running. Released.
- **opencode (this session, 2026-11-22, fifteenth): `bazzite`** — fix-loop
  session (HANDOFF item 27): scratch witness runs `mcpre1`–`mcpre4` /
  `mcpost1` under `work/config-103.toml` with the scratch stereo 4608-block
  library `work/media-103` (new dir, keep for future MediaCodec work),
  `build-atl` x4, full-corpus gates `fc27` vs `fc26` (0/0), `fc28` vs
  `fc27` (0/0) and `fc29` vs `fc28`. Fork commits `709f7b66`, `a9c384e1`,
  `c5024620`, `a3c8388a`, `71b502f5` pushed to `fix/fixcov-crashes` +
  `local-fixes`. Phone attempted
  (`rig -r oneplus12 build`) but unreachable: `waffle.local` does not
  resolve (phone asleep/off mDNS). Bazzite left installed with
  the `71b502f5` tree. Released.
- **opencode (this session, 2026-11-22/23, sixteenth): `bazzite`** — fix-loop
  continuation (HANDOFF item 28): `build-atl` x67 and full-corpus gates
  `fc30`..`fc70` (74 apps each, each vs its predecessor; all 0
  regressed except `fc42` which FAILED on the item-131 regression and
  was fixed, and `fc48`/`fc52` which FAILED on the canary's flake (see
  the batch 179–193 and 224–238 entries: cleared by `fc49`, 0/0 vs
  `fc47`, and `fc53`, 0/0 vs `fc51`); `fc37` 2
  improved: phonograph EXITED->ALIVE, nudge CRASH->BLANK; `fc40` 1
  improved: me.zhanghai.android.files CRASH->ALIVE; `fc60` 1 improved:
  same canary CRASH->ALIVE after the harness masked `/etc/group` from
  the sandbox (D18, killed the JNI flake mode); `fc64` 1 improved: the
  canary's other (NPE) flake mode cleared again, CRASH->ALIVE. Fork
  commits `940749c9`..`f52ecb55` (21 queue fixes + getWindowToken + fragment
  onActivityCreated dispatch + fillType + Ringtone + dispatchKeyEvent +
  View state batch + TextView isSingleLine + batches 119-133, 134-148,
  149-163, 164-178, 179-193, 194-208, 209-223, 224-238, 239-253,
  254-268, 269-283, 284-298, 299-313, 314-328, 329-343, 344-358 +
  hasWindowFocus fixup, 359-373, 374-388, 389-403, 404-418,
  419-433 and 434-448)
  pushed to `fix/fixcov-crashes` + `local-fixes`. Added
  `com.io7m.exfilac.main` to `corpora/fixcov.txt`; refreshed the queue
  with `atl-gaps --emit-queue
  15` after `fc39` (119 items), `fc41` (134), `fc43` (149), `fc44` (164),
  `fc46` (179), `fc47` (194), `fc49` (209), `fc50` (224), `fc51` (239),
  `fc53` (254), `fc54` (269), `fc55` (284), `fc58` (299), `fc59` (314),
  `fc61` (329), after 314-328 (344), `fc62` (359), `fc63` (374) and
  `fc64` (389), `fc65` (404), `fc66` (419), `fc67` (434) and
  `fc68` (449); the loop was stopped before the `fc69`/`fc70` refresh,
  with 434–448 of the 449-item queue consumed.
  Used `rig smoke` + single-hunk builds to
  bisect the item-63 witness and the item-131 regression, and repeated
  forced single-app runs (5 at `433b2354`, 8 at the new tip) to show the
  `fc48` canary CRASH is pre-existing flakiness rather than a regression,
  and 10 forced runs on *each* of the `7a869e95` and `e0f09030` trees
  (6/10 vs 5/10 CRASH, same signature) to show the same for `fc52`, and
  6 forced runs on each side of the `fc56` verdict (1/6 CRASH at the new
  tip vs 5/6 at `33f1fedd`, and the app's JNI-abort flake mode found in
  the *old* tree's logs too) for `fc56`-`fc58`; `fc59` was 0/0 vs
  `fc58` with the same canary JNI mode in both; `fc61` vs `fc60`
  FAILED on the canary's other (NPE) flake mode again — 10 forced
  runs at the new tip (7/10, same signature, and the canary never
  reaches the batch's changed code) vs 3/6 at the old tip: flake.
  Checked `rig -r bazzite
  job status` before starting; no other agent's
  jobs. Phone still unreachable (`rig -r oneplus12 doctor` FAIL ssh),
  phone items 67/103 stayed parked. Bazzite left installed with the
  `f52ecb55` tree (= `local-fixes` tip; `fc69`/`fc70` ran on it).
  Released.
- **opencode (this session, 2026-11-23): `oneplus12`** — the phone is
  back (`rig -r oneplus12 doctor` green over the LAN alias). Built the
  current tip on it: `rig -r oneplus12 build --detach --worktree
  ~/Develop/atl-fixcov` (21 s, BUILD OK), so the phone now runs
  `local:f52ecb55` in `/home/phone/src/atl-dev`, same tree as bazzite.
  Doctor re-checked: `atl-installed local:f52ecb55`. This unblocks the
  parked phone items (67 siggen SIGSEGV, 103 FLAC stutter) — not yet
  run. No app runs on the phone beyond the build itself; runner
  released.
- **opencode (this session, 2026-11-23, second): `oneplus12`** — Gramophone
  pause->resume freeze repro (work/gramo-resume/TODO.md item 1). Found the
  harness env on the phone had no PipeWire socket (rig `bwrap_prefix` did
  `--tmpfs /run`), so every ATL AudioTrack init failed "Host is down" and
  music runs only exercised a dead-audio retry loop; fixed by ro-bind-try of
  `pipewire-0`/`pulse/native` in `scripts/rig` bwrap_prefix. Scratch runs
  `grres1..grres6` via `work/config-grresume.toml` +
  `work/interactions-mp/org.akanework.gramophone.txt` against the owner's
  real `/home/phone/Music`: `grres3` reproduced the owner wedge (after the
  first pause/resume cycle, later pause/resume taps produce zero
  `AudioTrack` calls and the UI goes silent-frozen). Phone is shared with
  the owner's own desktop sessions (`atl-app` -> `atl-sandbox`); only
  harness Gramophone processes were touched. Also probed host PipeWire
  (read-only pw-dump) for the volume-key issue (TODO item 2): Mobian
  CallAudio role-sink setup, ATL ALSA streams carry no `media.role`.
  Phone still installed at `local:f52ecb55`.
- **opencode (this session, 2026-11-23, third): `oneplus12` + `bazzite`** —
  element addressing P0 (media transport keys) AND the owner's
  pause→resume bug, both fixed. Fork branch `fix/fixcov-crashes`
  (`~/Develop/atl-fixcov`): `dc58bf2a` session registry +
  `atlDispatchMediaKey` + capture-phase window key controller; witness
  `grres18` then reproduced the resume bug deterministically (no
  `AudioTrack.play()` after resume); `e20d7f16` fixes it —
  `getPlaybackHeadPosition` is now monotonic (PipeWire delay > frames
  written across `snd_pcm_pause` used to rewind the head to 0, media3
  saw a phantom `audioTrackUnderrun` and never re-primed the track:
  Loader stuck paused, ExoPlayer BUFFERING forever; thread stacks
  witnessed via temporary probes); `a52665e6` DOWN-only PLAY_PAUSE
  expansion + media3 legacy state numbering (legacy BUFFERING=6 —
  media3's vendored `session.legacy.PlaybackStateCompat` renumbers
  states). Witness after fix: `grres22..grres24` two pause→resume
  cycles, `play()` each resume, position advances 8158→15581 ms.
  Harness: semantic `key media-*` names (`8fd0656`), rig PipeWire socket
  bind (`64be560`). Gates (bazzite, corpora/fixcov.txt): `fc70→fc71`
  0 improved/0 regressed (keys only), `fc71→fc72` and `fc70→fc72`
  **0 improved/0 regressed** (tip `a52665e6`, covers the AudioTrack
  change). Bazzite and phone both installed with `local:a52665e6`.
  **Merged to `local-fixes` (tip `a52665e6`) and pushed to the fork's
  origin**; nothing went to upstream (push URL disabled).
- **opencode (this session, 2026-11-23, fourth): workstation (laptop)** —
  desktop PoC: Gramophone as a window on the laptop's Wayland session,
  no runner, no harness, no host install (sudo needs a password the agent
  has no access to): rootless **podman** container `atl-desktop` (image
  `atl-desktop-poc:latest` via `podman commit`) built by
  `scripts/runner/bootstrap.sh` *inside* the container (steps: packages
  wolfssl bionic art opensles atl; fork worktree `~/Develop/atl-desktop-poc`
  @ `a52665e6` bind-mounted, no fork changes). Recipe + gotchas in
  **docs/desktop-poc.md**. Witnessed on the live session (noctalia,
  Wayland + PipeWire sockets mounted): native window, MediaStore scanned
  `~/Music` read-only (4852 files probed), playback resumed mid-track and
  played through the laptop speakers (state PLAYING, position advancing,
  one `AudioTrack.play()` at start). **Media-key press-through on the
  desktop witnessed too** (dispatch origKey=85/87, next -> re-primed
  track, resume fix holds): the catch was the compositor - the owner's
  session is **umbriel wrapping noctalia**, and umbriel binds
  XF86Audio* to `noctalia msg media ...` (MPRIS) by default; with those
  three binds commented out the keys reach the window and ATL's
  capture-phase controller (verified with a temporary C probe). Config
  restored after the test. PoC gotcha: an ATL instance killed without
  procps (no pkill in fedora:44 base image) keeps running as a zombie
  whose audio/queue confuses what the new window shows - `dnf install
  procps-ng` first.
- **opencode (this session, 2026-11-23, fifth): workstation + bazzite +
  oneplus12** — the owner's boot complaint from the PoC ("music starts
  playing when the app is opened, and is skipping... a song should not
  play at all on boot") root-caused to ATL, not the app: the AudioTrack
  constructor set ALSA `start_threshold = one period`, so media3's
  pre-roll write for the restored queue started the stream by itself —
  on the PoC `wpctl status` showed the stream `[active]` with **zero**
  `AudioTrack.play()` calls and 94 `snd_pcm_recover underrun` lines; the
  phone's `grres21` had auto-played the same way (state=3 + advancing
  head at boot, no keys). Fix `8faa3ec3` (fork `local-fixes`): threshold
  = whole buffer + per-track `started` flag that re-pauses a stream a
  server started on write anyway. Witnessed audibly and in logs: desktop
  70 s untouched boot = constructor only, 0 underruns, owner's click
  plays cleanly from the pre-buffered frames; `grres25` (phone,
  `local:8faa3ec3`) = 52 s key-free window with no `play()`/no PLAYING,
  then scripted pause→resume plays normally (8113→15538 ms). Gate
  (bazzite, corpora/fixcov.txt): `fc72→fc73` 0 improved / 0 regressed.
  Runner use: `rig build` + gate on **bazzite**, witness build on
  **oneplus12** (both installed `local:8faa3ec3`), diagnosis + witness
  in the workstation's podman container `atl-desktop`. No ATL probes left
  in the tree; fork `MediaSession` witness line is from `a52665e6`.
  Same session: **D19 executed (identity level)** — fork repo renamed to
  **github.com/Alaxend0l/andrun**, README/FORK retitled "Andrun" with
  disclaimer + upstream credit (fork `16701e67` on `local-fixes`);
  pre-rename string scan findings recorded in D19/FORK.md. Mechanical
  renames (binary, meson, desktop entries, config strings) deliberately
  left as their own task — see D19.
- **opencode (this session, 2026-11-23, sixth): workstation + bazzite +
  oneplus12** — **P1 of element-addressed interaction scripts done,
  witnessed, merged** (brief:
  `docs/agent-tasks/element-addressed-interaction.md`; roadmap section
  updated). Fork `feat/element-dump`: `ATLElementDump.java` — view-tree
  dump to `views.json` behind env `ATL_ELEMENT_DUMP=DIR` (unset ⇒ no
  thread at all), hooked from the `Window` ctor, walk posted to the main
  looper; `+3694d263`, regenerated JNI headers `+0b1c3695` (fresh-sync
  builds were failing: committed `android_graphics_*.c` include
  Shader/Gradient headers that were never added). Merged to
  `local-fixes` as **`56206f55`** (merge message records gate + witness).
  Harness (`src/interact.rs`, `src/runner.rs`): steps
  `find`/`wait-find`/`tap $el`/`long-press $el`, exact-text-over-
  substring matching, off-Xvfb-screen nodes never matched (ViewPager
  neighbours sit at x>1080/x<0 in the dump), window-origin mapping via
  `xdotool`, failed finds list the app's exposed elements, matched tree
  kept as `elements.json`; 25 unit tests, clippy clean. Runner use:
  `rig build --worktree ~/Develop/atl-desktop-poc` + gate **fc75**
  (bazzite, corpora/fixcov.txt): **0 improved / 0 regressed** vs fc73;
  witness on **oneplus12**: run **p1w11** `interact_ok=1` on
  `org.fossify.musicplayer` — tab taps by element name (2.4–2.7% screen
  changes) then `tap $el` on track "Big Fig Wasp" starts playback and
  opens Now Playing, final assertion `wait-find "Bilayer"` matches the
  "Next: …" queue row (`p1w10` was the same run ending in
  `expect-text "Big Fig Wasp"` — fails, because Fossify draws the title
  only inside the **rotating album cover**, its title TextView marquees
  off-screen; lesson: OCR assertions fail on decorated text, prefer
  element finds. The fork merge message `56206f55` still names
  p1w10/expect-text; the accurate record is this brief + p1w11). Both
  runners left installed with `local:0b1c3695` (identical code to merged `local-fixes`),
  released. Follow-ups in the brief: Fossify row **subline** TextView
  taps register but do nothing (title taps work — parent-handled rows,
  fork `performClick` hits the leaf only); Gramophone is Compose and
  needs P3 a11y; P2 = convert tracked `interactions/` scripts.
  Note: the dumper commit once vanished from the shared clones (branch
  gone locally and on origin) — recreate + `git ls-remote` verify after
  pushing in shared repos.
- **opencode (this session, 2026-11-23, seventh): `mini` host (no ATL build)**
  — ran the two macOS evidence steps for the exe/DMG model (see
  `docs/research/vm-launcher-distribution.md` §8): cloned Lima disk booted
  under vfkit and a hand-written Swift VZ launcher (`~/andrun-vz/`, artifacts
  now in `scripts/research/`), plus the audio A/B that found mini's real audio
  root cause (**Lima attaches no audio device to the guest**; the cloud image
  also lacked `kernel-modules`, so no `virtio_snd.ko`). No `/usr/local`
  changes on mini; `atl` stopped during the spike and restarted afterwards
  (now Running, host keys regenerated — `ssh-keygen -R atl-mini-vm` done on
  the workstation). The spike VM (4 GiB) and `atl` (12 GiB) were never up at
  the same time on the 16 GB mini. Released.
- **opencode (this session, 2026-11-23, eighth): workstation only (no runner)**
  — finished the jvmrig brief (D21 option C): `genshim.py` auto-shim
  generator (1029 recording stubs) + 5 new test classes, all PASS against
  fork `main` `4a97b41b` via a temporary worktree `~/Develop/atl-jvmrig`
  (removed after). Found rig gap #1 (`KeyEvent.changeTimeRepeat` misses
  FLAG_LONG_PRESS) — fork-fix candidate, not touched here.
- **opencode (this session, 2026-11-23, ninth): `oneplus12`** — P2 of the
  element-addressed-interaction brief: harness `element` failure kind
  (`element not found: NAME`) + converted `interactions/org.fossify.{math,
  notes,musicplayer}.txt` to element-addressed steps (Gramophone annotated
  Compose-blocked). Witnessed on the phone (installed build `local:0b1c3695`
  already had the P1 dumper — no build, no installs): runs `p2a`
  (math/notes `interact_ok=1`; musicplayer fails only at the known
  `ExpectText("TestList")` step, `interaction` kind, line 31) and scratch
  `p2neg2` (bogus `wait-find` recorded `element` kind). Scratch config +
  scripts under `work/` on the runner. No full-corpus gate (harness-only
  change, scripted apps all witnessed). Released.

## Triage: local model vs Claude
`docs/agent-tasks/triage.md` (7 agents, every audit item + a survey): 65
items small enough for the local model (with precise fix hints, in
`docs/agent-tasks/local-queue.json`, which agent-loop now uses by default), 91
for Claude. Trial 3 of the loop: 2 commits; the null-listener fix was right
(merged), `Builder.setView` was plausible but wrong (needs a slot in the native
dialog layout; tag `agent-trial3` in the agent clone keeps it). Full overnight
run `20261121-185746`: 18 fixes reviewed and staged on `local-fixes`
(fork item 15, D17 — no local-model fix goes to `main` without owner
review); both gate-rejected tasks re-done by hand, also on `local-fixes`.
The agent branch was reset to the fork's main afterwards.

## Overnight agent (opencode + strata)
`scripts/agent-loop` ([docs/agent-loop.md](docs/agent-loop.md)): the local
model fixes audit items in `~/Develop/atl-agent` on branch `local-fixes`
(since D17; before that `agent/overnight`);
the agent can only edit `src/` there (permissions tested), the script builds,
gates, and commits/pushes only that branch when nothing regressed. Run on the
workstation: `scripts/agent-loop --hours 8`; review
`work/agent/<time>/summary.md` and the branch in the morning; add merged fixes
to `docs/audit/fixed.txt`.

## Tools added today (all in docs/rig.md)
`rig` (runners, sync, atlh, jobs, bootstrap, rootfs, build, smoke, desktop-apps, doctor), `rig build --worktree` (2–7 s edit→installed, parsed errors), bounded waits (exit 124 timeout / 125 stalled / 126 unreachable; every subprocess has a time limit), `atlh missing-apis`, `scripts/phone-shot`, `corpora/*.txt`, microG data templates, `scripts/atl-gaps` (static stub extractor + queue refills, docs/usage.md).

## Lessons (cost time today)
- `pkill -f PATTERN` from a shell whose command line contains PATTERN kills that shell itself (exit 144/255). Use `pgrep` + exact PIDs.
- Piping a run into `head` kills it (SIGPIPE). Run long things as `rig ... --detach` jobs.
- `rsync -a` copies mtimes: with clock skew cargo skipped rebuilds. `rig sync` doesn't copy mtimes.
- Branch switches while a worktree build/sync runs produce mixed trees: one branch per git worktree when running things in parallel.
- An app still dumping threads after an ART abort looks "alive"; the harness now classifies that as CRASH. ATL's light window frame made blank dark screens look non-blank; the harness crops it.
- The `!` prompt in Claude Code has no TTY: `sudo` there can't ask for a password.
- Background waits outside `rig job wait` can hang silently: always use bounded waits.
- `limactl stop`/`start atl` regenerates the guest's ssh host keys (cloud-init): clear the stale `atl-mini-vm` entry (`ssh-keygen -R`) after any Lima restart.
- Lima attaches **no audio device** to its guests — a Lima VM is not an audio test bed; vfkit v0.6.4 has no sound device either. For VM audio use VZ's `VZVirtioSoundDeviceConfiguration` (macOS 15+) directly (spike: `scripts/research/vzsnd.swift`).
- Audio-group grants don't apply to an already-running systemd user manager: PipeWire keeps seeing zero ALSA devices until the guest reboots (or the manager is restarted from a fresh login).
