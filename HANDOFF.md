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
| ATL fork | github `Alaxend0l/android-translation-layer`, private; workstation clone `~/Develop/android-translation-layer` | `main` = `7cb4cd2b`; upstream fetch-only. Runners pull with read-only deploy keys (alias `github-atl-fork`). Beware: several agents share this clone — one loop switches branches in it live (it also checks out `main` in `~/Develop/atl-shaders` and keeps a detached scratch worktree under `/tmp/claude-*/scratchpad/atl-main`); it silently discarded uncommitted edits once this session. Commit WIP early; never rely on uncommitted files surviving. |
| fedora-atomic-phone | github `Alaxend0l/fedora-atomic-phone`; clone `~/Develop/fedora-atomic-phone` | branches **`plasma-mobile-spectacle`** and **`oneplus12-flatpak-home`** pushed, not merged |

## ATL fork: what's merged on `main`
All gated with the corpus (52 apps on bazzite) unless noted:
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
left: AudioTrack `stop()`/`flush()` stubs (harness flags
`stub: STUB: AudioTrack.flush()`), `getTimestamp` returns false,
`AnimationUtils.loadInterpolator` returning null (crashes Music's progress
indicator), and mini's Lima audio device failing `snd_pcm_hw_params`
(VM config, audible audio untested there). Phone gate with the new main not
run yet.
1b. **Interaction scripts** exist (`interactions/`, docs/usage.md): Calculator (7+8=15), Notes (typing) and Fossify Music (library, playback, + a failing `setView` dialog check) are gated. Status and candidates: `docs/agent-tasks/test-first.md` "Status" (maxLines/ellipsize needs an OCR-DSL step first, e.g. `expect-no-text`). Add scripts for more apps, especially ones exercising fixes 5–8 (checkbox clicks, Enter/IME actions, preferences).
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
- Telephony/SMS design for Messages and Phone.
- Merging the two fedora-atomic-phone branches (`plasma-mobile-spectacle`, `oneplus12-flatpak-home`); both are already applied on the phone by hand.
- **Phone music playback check** (roadmap step "phone gate"): phone ATL is still at `8a511be7`; it needs `rig -r oneplus12 build --pull` (fork main `7cb4cd2b`), then open Fossify Music and play a track — I inspect `~/.local/state/atl/org.fossify.musicplayer.log` on the phone afterwards (audio may stay silent: Phone 12 raw-HW path caveat).

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
