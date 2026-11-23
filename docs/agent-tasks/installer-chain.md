# Installer chain: stores and installers as ordinary apps

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

Raised by the owner 2026-11-23 as a future-important capability: **installing
apps *using* apps** — F-Droid client, Obtainium (Aurora later). Today ATL only
installs from the outside (`--install`, copy to `_installed_apks_`); nothing
in the corpus has ever pressed an **Install** button. Status: **unverified —
do the evidence step first, then the design.** Roadmap §"Later: app stores and
installers" keeps the security staging (sandbox → mediated installs →
permission model → app isolation); this brief is the *measurement* half.

## Witness apps (`corpora/stores.txt`)
- `org.fdroid.fdroid` — already in `corpora/fixcov.txt`, ALIVE in the fc
  gates (its WindowInsets noise was killed in fork item 22). Install methods
  to expect in its logs: intent-based (`ACTION_VIEW` + FileProvider URI),
  **root** (`su`/`pm` — will never work under ATL, expect and classify it) and
  **accessibility**-based auto-clicking (needs harness P3/a11y, so a
  fallback, not the path).
- `dev.imranr.obtainium` — simplest target: plain
  `Intent(ACTION_VIEW, content://…)` install requests on APK files
  (plus Shizuku/root modes that won't apply). Start here: one APK download +
  one install intent is the smallest end-to-end witness.

## Evidence step (first, cheap)
1. `atlh fetch --pkg dev.imranr.obtainium` (+ confirm `org.fdroid.fdroid`) on
   a runner; run both; diff signatures vs the known 127-sigma baseline.
   Expect the first truths to be in: `getPackageInfo`/`queryIntentActivities`
   (installed-version checks → update UI), `PackageInstaller` createSession
   surface, `ACTION_VIEW` for `application/vnd.android.package-archive`
   (nothing handles it → app shows "no app can open"),
   `REQUEST_INSTALL_PACKAGES` consent state.
2. Only after that: an interaction script that goes F-Droid/Obtainium →
   download a tiny F-Droid APK (e.g. `org.secuso.privacyfriendlytodolist`) →
   press Install. **Pass = an installer consent UI appears** (that's the
   mediated-installer door, not the install itself); the full pass is the app
   appearing on the desktop afterwards (`rig desktop-apps`).

## What the chain needs (from the ATL code read; re-verify in step 1)
- An **installer UI as a real app/dialog** (ATL has none): the consent screen
  the intent path must land in.
- `PackageInstaller` session API or at least the intent path: APK handover
  via a `content://` URI readable across sandboxes (FileProvider + portal
  read), then commit → D-Bus/portal to the trusted installer outside the
  sandbox (roadmap order: sandbox first, or mediated installs buy nothing).
- **Signature continuity** on updates (verify APK key matches the installed
  app) — this is the part nobody can fake in a harness gate.
- Package queries: installed versionCodes for "Update" badges and for
  F-Droid's repo diffing (repo fetch itself is plain HTTP — fine today).
- Launch-after-install: spawn the new APK as its own ATL process (already
  how `--install` + desktop entry works).
- Not blocking, later: notification/download-manager polish for F-Droid
  background updates; a11y install as fallback path.

## Constraints from elsewhere
- **Launcher design (D22)**: in the VM products the whole chain must work
  *inside the guest* — the drop-directory daemon is a dev convenience, not
  the product's install path; the mediated installer is what ships.
- **Sandbox rule (AGENTS)**: do not widen any sandbox policy to make a store
  "work" (e.g. letting F-Droid write `_installed_apks_` directly is exactly
  the hole the roadmap refuses to open).
- No root (`su`) under ATL, ever: F-Droid/Obtainium root modes must classify
  as unsupported, not be emulated with a fake `su`.

## Open questions for the owner (later, not now)
- Where the consent UI lives: a GTK dialog from ATL itself vs a real
  `PackageInstaller` system app APK (more Android-faithful, testable in the
  corpus, but needs system-app plumbing).
- Whether "install" means desktop entry + `_installed_apks_` only, or also
  an in-guest "app drawer" surface for the VM products.
