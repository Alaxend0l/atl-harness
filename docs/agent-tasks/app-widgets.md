<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: app widgets (AppWidgetManager, RemoteViews, a widget host)

Unblocks: **Flow** (exits on `AppWidgetManager.getInstalledProviders()`),
**Fossify Launcher** (a widget host), and later showing apps' widgets on the
phone's Plasma Mobile home screen. Today ATL has stubs only
(`src/api-impl/android/widget/RemoteViews.java` 40 lines,
`android/appwidget/AppWidgetManager.java` 19 lines).

Do it in stages. Each stage is useful on its own and is gated
(`rig atlh run` + `compare`); write a failing interaction script first where
there is something visible (docs/agent-tasks/test-first.md).

## How Android does it (what to reproduce)
- A **provider** app declares a `<receiver>` with an intent filter for
  `android.appwidget.action.APPWIDGET_UPDATE` and
  `<meta-data android:name="android.appwidget.provider" android:resource="@xml/...">`
  (an `<appwidget-provider>` XML: minWidth, minHeight, updatePeriodMillis,
  initialLayout, previewImage, resizeMode, widgetCategory, configure).
- A **host** (launcher) asks `AppWidgetManager` for providers, allocates an
  id (`AppWidgetHost.allocateAppWidgetId`), binds it, and shows an
  `AppWidgetHostView`.
- The provider receives `APPWIDGET_UPDATE` broadcasts and pushes
  **RemoteViews** (a layout id from the provider's package plus a list of
  actions: set text, set image, click PendingIntent, visibility,
  reflection setters, collection adapters) via `updateAppWidget(id, views)`.
- The host inflates the layout **from the provider's resources** (not its
  code) and applies the actions. Clicks fire the provider's PendingIntents.

## Stage 1: the API for widget providers (local model)
Real behaviour of a device where no widget has been placed yet:
- `AppWidgetManager.getInstance(Context)`.
- `getInstalledProviders()` / `getInstalledProvidersForProfile(...)`
  / `getInstalledProvidersForPackage(pkg, user)`: the loaded app's own
  receivers that have the APPWIDGET_UPDATE filter and the
  `android.appwidget.provider` meta-data, from ATL's parsed package
  (`ATLLoadedApp.getPrimaryApplication().pkg.receivers`, see how
  `PackageManager.queryBroadcastReceivers` reads them), each as an
  `AppWidgetProviderInfo` filled from the provider XML (parse it with the
  app's `Resources.getXml(id)`; attribute names as in AOSP's
  `AppWidgetProviderInfo`).
- `getAppWidgetIds(ComponentName)` returns `new int[0]` (nothing placed).
- `getAppWidgetInfo(id)` null for unknown ids; `getAppWidgetOptions(id)` an
  empty Bundle.
- `updateAppWidget(int[]|int|ComponentName, RemoteViews)`,
  `partiallyUpdateAppWidget`, `notifyAppWidgetViewDataChanged`: store the
  latest RemoteViews per id (map), nothing to draw yet (no host).
- `isRequestPinAppWidgetSupported()` false; `requestPinAppWidget(...)` false
  (both what Android returns when the launcher can't pin).
- `AppWidgetProviderInfo` as a real class (fields + `loadLabel`,
  `loadIcon`, `loadPreviewImage`), `AppWidgetProvider.onReceive` dispatching
  to onUpdate/onDeleted/onEnabled/onDisabled/onAppWidgetOptionsChanged as AOSP.
Done when: Flow starts (corpus: add Flow's APK, `rig atlh run --pkg ...`),
no `NoSuchMethodError` from android.appwidget in `atlh missing-apis`, gate
0 regressions.

## Stage 2: RemoteViews for real (local model, after a design review)
- Store actions like AOSP's `RemoteViews.Action` subclasses: `setTextViewText`,
  `setTextColor`, `setImageViewResource/Bitmap/Uri`, `setViewVisibility`,
  `setOnClickPendingIntent`, `setOnClickFillInIntent`, the reflection
  setters (`setInt/Boolean/Float/String/CharSequence/Uri/Bitmap/Intent`,
  call a named public setter), `setRemoteAdapter` + `RemoteViewsService`,
  `addView/removeAllViews`, `setProgressBar`, `setChronometer`.
- `apply(Context, ViewGroup)` / `reapply`: inflate `layoutId` with a context
  for the **provider's package** (in-process at first: same app), run the
  actions. Only the view classes Android allows in RemoteViews
  (`@RemoteView`: FrameLayout, LinearLayout, RelativeLayout, GridLayout,
  TextView, ImageView, Button, ImageButton, ProgressBar, Chronometer,
  ListView, GridView, StackView, AdapterViewFlipper, ViewFlipper, ...).
- Parcelable (writeToParcel/CREATOR) so stage 3 can move them between
  processes; a round-trip unit-style check through `Parcel`.
- `AppWidgetHost` + `AppWidgetHostView` inside one process (a launcher showing
  its own widgets): allocate/bind ids, `createView`, `updateAppWidget` ->
  `apply`.
Test: an interaction script on an app that shows its own widget preview or
uses RemoteViews for notifications (also used by notification layouts).

## Stage 3: widgets across apps and on the Linux desktop (designed 2026-11-21)
Providers and hosts are different apps in **different sandboxes**
(docs/sandbox.md). Design, built from patterns ATL already uses (UnifiedPush
`Connector1` in `android_content_Context.c`, MPRIS in
`android_app_NotificationManager.c`: gdbus-codegen XML + skeleton + own name):

**Pieces**
1. `atl-widgets`: a small trusted session service **outside** the sandboxes
   (C + GDBus, in the fork as `src/atl-widgets/`, built by meson; started by
   D-Bus activation). Owns `org.atl.Widgets1`. State in
   `~/.local/share/atl-widgets/state.json` (placed widgets: id, provider
   package + receiver, host package, size, options) plus the latest
   RemoteViews per id as Parcel bytes (`<id>.rv`).
2. Provider side (in ATL, each app): `AppWidgetManager.updateAppWidget(ids, rv)`
   marshals `rv` to a Parcel (stage 2) and calls
   `org.atl.Widgets1.Update(ids, bytes)`. The app exports
   `org.atl.WidgetProvider1` on **its own package name** (the sandbox
   already allows owning it): `Update(ids)`, `Deleted(ids)`, `Enabled()`,
   `Disabled()`, `OptionsChanged(id, a{sv})`, `Click(id, viewId)`; each
   becomes the AOSP broadcast to the app's AppWidgetProvider receiver.
3. Waking a provider that isn't running: D-Bus activation.
   `rig desktop-apps` writes `~/.local/share/dbus-1/services/<pkg>.service`
   (`Exec=…/atl-app …/<pkg>.apk --service`). ATL needs a **service mode**
   (`--service`): start the app (Application.onCreate, receivers) without
   an activity window, exit after ~30 s idle (GApplication service mode +
   `g_application_hold/release`). Also useful for UnifiedPush and alarms.
4. Hosts: a launcher in ATL (Fossify Launcher) uses `AppWidgetHost`;
   ATL's implementation talks to `org.atl.Widgets1`: `Allocate()`,
   `Bind(id, provider)`, `Get(id) -> (bytes, apk fd)`, signal
   `Updated(id)`, `Click(id, viewId)`, `Delete(id)`.
   A desktop host (`atl-widget-window PKG/RECEIVER`, ATL started with a
   built-in host activity showing one AppWidgetHostView) shows any widget
   as its own window on Plasma Mobile.

**Security rules**
- Identity: the service never trusts names in messages. A caller's package is
  the well-known name it owns (`GetNameOwner(pkg) == sender`, which only the
  app's own sandbox can own). Providers may only update their own widgets;
  only the host that bound an id may read, resize or delete it.
- Rendering without running provider code: `Get` passes the provider's APK
  as a **file descriptor** (GDBus `h`, fd passing works through
  xdg-dbus-proxy). The host's ATL opens a resources-only package context from
  `/proc/self/fd/N` (AssetManager over the fd, no class loader) and inflates
  with a LayoutInflater filter that only allows `@RemoteView` classes, as
  Android does. No provider code ever runs in the host.
- Clicks: the host sends `Click(id, viewId)` to the service, which relays
  it to the provider (activating it if needed); the provider fires its own
  PendingIntent. PendingIntents never cross sandboxes.
- Sandbox policy: every app may `--talk=org.atl.Widgets1`; only
  `atl-widgets` may call `org.atl.WidgetProvider1` (the proxy's
  `--call` rules restrict the provider side to calls from the service).

**Order of work** (after stages 1 and 2):
3a. ATL `--service` mode (no window, idle exit) + service files from
    `desktop-apps`; check: UnifiedPush or a widget provider wakes from D-Bus.
3b. `atl-widgets` service with the state file and identity checks; unit-test
    the state logic without D-Bus.
3c. Provider side in ATL (export WidgetProvider1, `updateAppWidget` → service).
3d. Host side in ATL (`AppWidgetHost` over the service; resources-only
    package context from an fd; restricted inflater).
3e. `atl-widget-window` desktop host + launcher entry.
Collections (`RemoteViewsService`, ListView widgets) come last: the host asks
the service, which binds the provider's RemoteViewsService in the
provider's process and relays item RemoteViews.

Each step is gated and has its own check; 3b and 3d are security-sensitive:
get them reviewed before merging.
