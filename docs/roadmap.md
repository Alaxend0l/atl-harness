# Roadmap and status

_Last updated 2026-11-21._

## Status

| Stage | State |
|---|---|
| 1. Census: fetch, run, classify, rank | **Written, unit-tested, not yet run against real ATL** |
| 2. Reference oracle (Waydroid) | Designed only |
| 3. Regression gate | `compare` implemented; no flake handling yet |
| 4. Agent loop | Prompt generation (`workitem`) implemented; orchestrator not started |

Tested so far: classifier parsing and rank/compare SQL (`cargo test`).
End to end on the box (x86_64 Bazzite, `atl` distrobox, upstream ATL
`22a96b28`): `fetch` against the live index, `run` under Xvfb, `rank`. A
3-app smoke run gave 3 × BLANK with 6 signatures, no blind spots.

## Next steps, in order

1. **Box bring-up.** Install packages and Rust, build upstream ATL, `sync`
   and build atlh. ([setup.md](setup.md)) *Done.*
2. **First census.** `fetch --limit 20`, `run`, then work through the
   [first-run checklist](setup.md#first-run-checklist): ATL command, Xvfb
   rendering, stub patterns, blank threshold, blind spots. Update config
   and classifier, then scale to 100–500 apps.
3. **Create the ATL fork repository.** Clone upstream, add it as the
   `upstream` remote, push to a private repo of your own, and point
   `[agent].atl_repo` at the box checkout. Record upstream's license.
   *Repo created (2026-11-21): `github.com/Alaxend0l/android-translation-layer`,
   private, `main` = upstream `22a96b28` + fork notice; upstream is
   fetch-only; license GPLv3. Workstation clone: `~/Develop/android-translation-layer`.
   Runners pull it with per-runner read-only deploy keys (alias
   `github-atl-fork`) and build `main`. Done.*
4. **Sandbox every app (priority: early).** Run ATL under bubblewrap per
   app: private home/data dir, no access to other apps' data or
   `_installed_apks_`, no LAN access (or none at all), only the X socket
   and GPU nodes it needs. Protects the runners, their keys and the LAN
   from untrusted APKs, makes results more repeatable, and is the
   foundation for installers (below). Upstream ATL lists bubblewrap on its
   own roadmap. Proven pieces: unprivileged `bwrap` works on all runners'
   kernels; `rig`'s `bwrap_root` (oneplus12) already runs everything in a
   bwrap'd rootfs with a private session bus. Next: a per-app `bwrap`
   wrapper in `src/runner.rs` behind a config switch, A/B against an
   unsandboxed run (`atlh compare`) to see which apps need what.
5. **Flake handling.** Make `compare` (or a new `gate` command) rerun
   regressed apps N times automatically and only fail on consistent
   regressions.
6. **Signature history.** Per app, track how the top signature changes
   between runs so "crash moved deeper" counts as progress.
7. **Agent orchestrator** (`atlh agent`): take the top signature not yet
   attempted, create a worktree, write the work item, run `[agent].command`,
   build ATL, rerun affected apps, then the full corpus, then the gate. Push a
   branch to the fork on success. Record attempts in a new table.
8. **Patch lint for agent diffs**: flag new `return null;`, empty bodies,
   and catch-all catches in framework code.
9. **Reference oracle**: run the corpus on Waydroid, mark apps that don't
   work there (or need Play Services) as out of scope, and later compare
   screenshots.
10. **Input scripting**: drive apps past their first screen (taps,
   scrolls), so later code paths get exercised.

## Later: app stores and installers (F-Droid client, Aurora, Obtainium)
Not checked yet; from the ATL code read so far, probably **not working**.

ATL has: `_installed_apks_` in the data dir, `--install`/`--install-internal`
(copy an APK there; `--install` also adds a launcher entry via the desktop
portal), and `ATLLoadedAppManager`, which lets one app see and load another
installed APK (how microG works).

A store also needs (unverified in ATL): the `PackageInstaller` session API
or the install intent; `REQUEST_INSTALL_PACKAGES` and its consent screen;
package queries reporting installed versions (for updates); launching the
newly installed app as a separate ATL process. If those are stubs, stores
download and then fail or silently do nothing at install. A harness smoke
test of F-Droid's client would settle it.

Doing it without opening holes, in this order:
1. **Sandbox first** (step 4 above). Today any app can write
   `_installed_apks_` directly and bypass any install flow.
2. **Mediated installs**: the app hands the APK over
   (`PackageInstaller.commit` → D-Bus/portal) to a small trusted installer
   outside the sandbox that asks the user to confirm every install/update
   (no silent installs), verifies the APK signature, enforces signature
   continuity (updates signed with the installed version's key), and only
   then copies to `_installed_apks_` and adds the launcher entry.
3. **Android's permission model**: only `REQUEST_INSTALL_PACKAGES` holders
   may ask; the user allows each installer source once.
4. **Isolation between installed apps**: separate data dirs, processes and
   sandboxes; no loading another app's code unless intended (GMS/microG).

Without the sandbox, a mediated installer adds features but no security.
For the harness, stores are late targets: they need network, background
downloads and notifications; Aurora also needs a Google login (see the
Play Store terms-of-service caveats discussed 2026-11-21).

## Known limitations
- Only startup is tested: one screenshot after `settle_s`, no input.
- Blank detection is a single std-dev threshold.
- `fetch` loads the whole F-Droid index into memory (tens of MB; fine).
- Long runs: `rig atlh --detach NAME ...` (jobs survive ssh drops).
- New Xvfb per app costs ~1 s; fine for now.

## Fork work queue (targets from census1, 2026-11-21)
Every failure recorded so far is in upstream ATL too: the fork is upstream
`22a96b28` plus a notice. Upstream's own test APKs (Gravity Defied,
gles3jni) already work, so they don't help. Based on only 43 app runs
(20 bazzite, 20 mini, plus re-runs); a 100–500 app census ranked with
`rig atlh rank` should reorder this. Work down from signatures that break
the most apps (`brk`) and that are an app's only blocker (`sole`).

**0. Fossify suite first (owner's priority: phone-essential apps).**
`corpora/fossify.txt`, run `fossify1` (2026-11-21), same on x86_64 and
aarch64: **12/16 ALIVE** with real UIs (Contacts, Calendar, Notes,
Calculator, Clock, Gallery, File Manager, Music Player, Keyboard, Paint,
Documents, Voice Recorder). Blockers, one per app:
- Messages: `Telephony.Sms.getDefaultSmsPackage(Context)` missing.
- Phone: `getSystemService("telecom")` not implemented (TelecomManager).
- Camera: `SoundPool.load(String, int)` missing (+ `setOnLoadCompleteListener` stub).
- Launcher: `android.appwidget.AppWidgetHost` missing.
Visible but unclassified: File Manager and Music Player show a black
rectangle top-left (likely an unrendered popup/overlay from the shared
Fossify library: one fix, several apps); Gallery opens a GTK file chooser
at start (ATL's storage-access mapping) that covers the app.

Update 2026-11-21 (harness now scores aborting apps as CRASH and crops the
window frame; baseline `fossify-base2` on fork `main`): **10 ALIVE, 6 CRASH**.
File Manager and Music Player were never really alive: both abort with a
JNI pending `NullPointerException` from `PropertyValuesHolder.setAnimatedValue`
(shared Fossify animation code) shortly after drawing: next family target.
- Camera: fixed on fork branch `fix/soundpool-path-load` (SoundPool
  `load(String,int)`, 1-based IDs, load callbacks; `setShowWhenLocked`/
  `setTurnScreenOn`). Gate `fossify-base2` → `fossify-fix2`: CRASH → BLANK,
  0 regressions. Blank is expected: no runner has a camera. Not merged to
  fork `main` yet (owner review).

**Owner's wishlist app: Flow** (github.com/Flow-Tube/Flow, YouTube/YouTube
Music client, Jetpack Compose; use the `flow-foss-*` APKs, sha256 from the
release's checksums.txt). Smoke on oneplus12 (v3.0.5): reaches `onResume`,
then exits 10 via its crash handler. Blockers:
- **ATL picks the wrong launch activity**: it takes the first `MAIN`
  activity (`TvActivity`, `LEANBACK_LAUNCHER`) instead of the
  `CATEGORY_LAUNCHER` one (`MainActivity`). Affects every app that also
  targets Android TV.
- `AppWidgetManager.getInstalledProviders()` missing (same widget area as
  Fossify Launcher's `AppWidgetHost`: one implementation helps both).
- `WindowInsetsCompat` reflection on `View.getViewRootImpl` fails (logged,
  non-fatal).

**1. Infrastructure bugs (first: they make every other result clearer)**
- **JNI calls with a pending exception.** ATL native code calls
  `GetMethodID`/`FindClass`/`GetObjectClass` while a Java exception is
  pending, so CheckJNI aborts (8/20 apps on the mini; signature
  `signal: SIGABRT: JNI … called with pending exception`). Fixing it turns
  hard aborts into readable Java errors. Hits Markor, Fossify Messages,
  SuperTuxKart, Daedalus, IonLauncher, among others.
- **microG native lib path.** `main.c:416` makes data dirs `<apk file>_`,
  `ATLLoadedAppManager.java:122` looks in `<package>_/lib`: in-process
  microG loads without its native libraries. One-line naming mismatch.

**2. Single-app targets, one missing API each (good first fixes)**
- Markor (`net.gsantner.markor`, popular notes app): `Window.setExitTransition`.
- Fossify Messages (`org.fossify.messages`): `Telephony.Sms.getDefaultSmsPackage`.
- SuperTuxKart (`org.supertuxkart.stk`): `addContentView`, then OpenGL; a real native game.
- Daedalus and other VPN apps: `android.net.VpnService` (likely hard to do for real).
- `Menu.addSubMenu(CharSequence)` (small); `Context.startForegroundService`.
- Torch app (`com.softcodingforyou.touchtorch`): `CameraManager$TorchCallback`.
- bazzite: `InputFilter$AllCaps`, `WebSettings.getJavaScriptEnabled`,
  `WindowInsets.getInsetsIgnoringVisibility`, `AccessibilityService`.
- `android.webkit.VideoView`/`CalendarView` lookups: the layout inflater
  can't find real `VideoView`/`CalendarView` widgets (probably missing).

**3. Families (one fix, many apps)**
- **Fossify suite** (Messages, Music Player, Gallery, …): shared library.
- **Flutter apps** (e.g. `com.sidhant.colorgram`): same engine abort in
  `library_loader.cc` (`[FATAL:…] Check failed: result.`). Common on F-Droid.
- **Godot games**: `libgodot_android.so` needs NDK camera symbol
  `ACameraMetadata_getConstEntry`.

**4. Play Services apps** (`corpora/microg.txt`: Mastodon, Tasks.org,
EVMap, …) don't reach Play Services yet; good tests once section 1 is fixed.

## Agent tasks
Standing work for coding agents, independent of the steps above:
- [Runner registry, harness tooling, documentation](agent-tasks/tooling-and-docs.md)
- [Research: compatibility layers (Wine, Proton, FEX, Berberis, …) and tooling](agent-tasks/research-compat-layers.md) (approved by the owner; report, don't adopt)
