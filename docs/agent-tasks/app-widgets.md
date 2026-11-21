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

## Stage 3: widgets across apps and on the Linux desktop (Claude: design)
Providers and hosts are different apps in **different sandboxes**
(docs/sandbox.md), so this needs a small trusted service. Direction:
- `atl-widgets`, a session service **outside** the sandboxes, owns the
  registry (`~/.local/share/atl-widgets/`: placed widgets, their provider,
  host, size, latest RemoteViews as Parcel bytes).
- Apps talk to it over D-Bus (one more name allowed by the sandbox's
  xdg-dbus-proxy: `org.atl.Widgets`); calls are checked against the caller's
  package (from the sandbox, not from the message).
- Rendering needs the provider's **resources** but must not run its code in
  the host: the host gets read-only access to the provider's APK resources
  (Android does the same with createPackageContext + restricted inflation).
  In sandbox terms: the service hands the host an fd of the provider APK.
- Updates: `APPWIDGET_UPDATE` (periodic / on placement) needs the provider app
  to run without an activity: ATL needs a "run this BroadcastReceiver and
  exit" mode (`android-translation-layer --receiver ...`), started by the
  service in the provider's sandbox.
- A plain GTK host (`atl-widget-window PKG/RECEIVER`) can show any widget as
  its own window on Plasma Mobile before launchers do.
Open questions for the design session: click PendingIntents crossing
sandboxes (service relays them), collection widgets (RemoteViewsService in
the provider's process), memory (one ATL process per updating provider).
