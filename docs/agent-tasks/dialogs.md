<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: dialogs — cancel/back semantics for every Dialog, and real slots in android.app.AlertDialog

For an interactive opencode session (needs building, running, screenshots).
Fork paths below are relative to `~/Develop/android-translation-layer`
(read at fork `main` = `9a2cd6c4`). Read "Hard rules" in `AGENTS.md` first.

## Symptom / why it matters
Two different code paths show dialogs in ATL, and they break differently:

1. **androidx.appcompat dialogs (most apps, all Fossify apps).** Fossify Commons
   builds every dialog with `androidx.appcompat.app.AlertDialog.Builder` (or
   `MaterialAlertDialogBuilder`) and `setupDialogStuff()`:
   `create().apply { setView(view); requestWindowFeature(FEATURE_NO_TITLE);
   setCustomTitle(title); setCanceledOnTouchOutside(cancelOnTouchOutside); show(); ... }`
   (Material variant: `setTitle(..); setView(view); setCancelable(cancelOnTouchOutside); show()`).
   appcompat's own `AlertController` inflates its layout in Java and calls
   `Dialog.setContentView()`, so it goes through ATL's **Java** `android.app.Dialog` +
   `android.view.Window` (decor FrameLayout), *not* through ATL's native AlertDialog.
   Proof: Fossify Keyboard's startup dialog (`work/remote/bazzite/runs/fossify-sp2/
   org.fossify.keyboard.png`) renders message + OK correctly (top-left, no WM).
   What is broken here: `setCancelable(false)` is ignored, `OnCancelListener` never
   fires, Escape/back does nothing to the dialog, theme `windowCloseOnTouchOutside`
   overrides the app's `setCanceledOnTouchOutside(false)`, `onCreate` runs on every
   `show()`, `dismiss()` fires `onDismiss` twice when called twice.
2. **Framework `android.app.AlertDialog` (older apps, and ATL's own
   `DatePickerDialog`/`TimePickerDialog`).** ATL replaces the dialog's content with a
   native GTK box (message label, list, buttons). `Builder.setView` is a no-op, so
   custom views (EditTexts, pickers) are missing; `AlertDialog.setView` puts the view
   into the decor that is no longer in the window (so Date/TimePicker dialogs very
   likely show only buttons — verify); `setSingleChoiceItems`/`setMultiChoiceItems`/
   `setAdapter`/`setCustomTitle` don't exist (NoSuchMethodError); `setItems` clicks
   don't dismiss; buttons are stacked vertically.

Fossify screens to use as checks: **Notes** "+" (menu item "Add a new note") opens
`NewNoteDialog`: custom title "Add a new note", custom view with an EditText (hint
"Label"), radio buttons "Text note"/"Checklist", buttons "OK"/"Cancel";
cancelable. **Keyboard** shows a message dialog with "OK" at startup.

## Current code (what is wrong)
`src/api-impl/android/app/Dialog.java`
- L81 `setCancelable(boolean)` and L83 `setOnCancelListener(..)` are empty.
- L93-136 `show()`: L98 calls `onCreate(null)` on *every* show (AOSP: once,
  `dispatchOnCreate` guards with `mCreated`); L105-107 applies the theme's
  `windowCloseOnTouchOutside` *after* the app's own `setCanceledOnTouchOutside()`,
  overriding it; no `onStart()`; no `showing` flag.
- L143-157 `dismiss()`: always posts close + `onDismiss`, even if not showing
  (double `onDismiss`); no `onStop()`.
- L217-220 `cancel()` just calls `dismiss()` (no `OnCancelListener`).
- L230 `onTouchEvent` returns false; there is no `onBackPressed()`, `onStart()`,
  `onStop()`, `setOnKeyListener` (L234) is empty. androidx `ComponentDialog`'s
  back fallback is `OnBackPressedDispatcher { super.onBackPressed() }`, i.e. it
  calls `android.app.Dialog.onBackPressed()`, which ATL lacks.
- L27 `nativeSetContentView` is declared but never called (harmless).

`src/api-impl-jni/app/android_app_Dialog.c`
- L11-17 `on_close_request` (window close button) calls `dismiss` and returns
  FALSE: ignores cancelable, never calls the cancel listener.
- L26-38 `on_touch_outside` calls `dismiss()` (should be cancel, and only when
  cancelable). It is a capture controller on the *main* window (L109-113), so a
  press anywhere on the main window while the dialog is visible counts as outside.
- L53-61 `nativeInit`: no key controller, so Escape never reaches the dialog.
  (View key controllers, `src/api-impl-jni/widgets/WrapperWidget.c` L480-487, are
  only installed for views that override `dispatchKeyEvent`/`onKeyDown`; L366 maps
  `GDK_KEY_Escape` to `KEYCODE_ESCAPE`.) The main window's back button
  (`src/main-executable/back_button.c` L11) goes to the Activity, not the dialog.

`src/api-impl/android/app/AlertDialog.java`
- L59-61 `setView(View)` -> `setContentView(view)` -> `Window.setContentView`
  (`src/api-impl/android/view/Window.java` L67-76: `decorView.removeAllViews();
  addView(view)`), but `nativeConstruct` already replaced the window child with
  the GTK box, so the decor is not shown. Forwarding `Builder.setView` here (trial 3)
  was wrong for that reason and because it would replace title/message/buttons.
- L105-107 `Builder.setCancelable`, L133-135 `Builder.setView`, L150-152
  `Builder.setOnCancelListener` are no-ops. No `setOnDismissListener`,
  `setView(int)`, `setCustomTitle`, `setSingleChoiceItems`, `setMultiChoiceItems`,
  `setAdapter`, `setIcon(Drawable)`, `setOnKeyListener`, `getContext` on Builder;
  no `findViewById` that sees the custom view.

`src/api-impl-jni/app/android_app_AlertDialog.c`
- L40-65 `nativeConstruct`: vertical GtkBox = message label (L48), GtkListView
  (L57), positive/negative/neutral buttons stacked vertically (L60-62), then
  `gtk_window_set_child(dialog, content_area)` (L64) detaches the decor view.
- L30-38 `activate_cb`: calls `OnClickListener.onClick` but never dismisses.
- Title: `Dialog.setTitle` sets the GTK window title (Dialog.c L63-69), shown in
  the CSD header bar; keep that.

## What Android does (quoted from AOSP android14 / androidx; no web needed)
Dialog.java:
```java
public void setCancelable(boolean flag) { mCancelable = flag; }          // default true
public void setCanceledOnTouchOutside(boolean cancel) {
    if (cancel && !mCancelable) mCancelable = true;
    mWindow.setCloseOnTouchOutside(cancel); }
public void cancel() {
    if (!mCanceled && mCancelMessage != null) { mCanceled = true; /* -> OnCancelListener.onCancel(this) */ }
    dismiss(); }
public void onBackPressed() { if (mCancelable) cancel(); }               // Back AND Escape keys
public boolean onTouchEvent(MotionEvent e) {
    if (mCancelable && mShowing && mWindow.shouldCloseOnTouch(mContext, e)) { cancel(); return true; }
    return false; }
show():  if (mShowing) return; mCanceled = false; if (!mCreated) dispatchOnCreate(null); onStart(); ... mShowing = true;
dismissDialog(): if (!mShowing) return; remove window; onStop(); mShowing = false; sendDismissMessage();
```
Theme `windowCloseOnTouchOutside` is only a default ("setCloseOnTouchOutsideIfNotSet").
OnDismissListener runs after OnCancelListener; a button click dismisses (not cancels).

AlertController layout (`alert_dialog_material.xml`), top to bottom:
`topPanel` (custom title view if set, else icon + `alertTitle`; hidden if no title)
-> `contentPanel` (ScrollView with `message`; if there is no message, the ScrollView is
replaced by the ListView; **message wins over items**) -> `customPanel` > `custom`
(FrameLayout; `setView` view added with MATCH_PARENT; panel GONE if no view)
-> `buttonPanel`, horizontal: `button3` (neutral, left), spacer, `button2`
(negative), `button1` (positive, right). Buttons with empty text are GONE.

Buttons: handler calls the listener (if any), then **dismisses** (posted).
Lists (`AlertParams.createListView`):
```java
setItems/setAdapter:   onClick(dialog, position); dismiss();          // no dismiss for single choice
setSingleChoiceItems:  CHOICE_MODE_SINGLE, item checkedItem pre-checked; onClick(dialog, position); NO dismiss
setMultiChoiceItems:   CHOICE_MODE_MULTIPLE; checkedItems[pos] = isChecked;
                       OnMultiChoiceClickListener.onClick(dialog, pos, isChecked); NO dismiss
```
appcompat `AlertDialog.Builder.create()`:
```java
dialog.setCancelable(P.mCancelable);              // default true
if (P.mCancelable) dialog.setCanceledOnTouchOutside(true);
dialog.setOnCancelListener(P.mOnCancelListener);
dialog.setOnDismissListener(P.mOnDismissListener);
if (P.mOnKeyListener != null) dialog.setOnKeyListener(P.mOnKeyListener);
```
appcompat `AlertDialog.onCreate()` calls `mAlert.installContent()` (inflates the
layout, adds the custom view): calling `onCreate` twice re-adds a view that already
has a parent -> IllegalStateException.

## Implementation steps (in order of user impact; build + gate after each group)
Rules: `dx` = no lambdas / method refs, use anonymous classes. A new Java `native`
method needs its JNI declaration added by hand to
`src/api-impl-jni/generated_headers/android_app_<Class>.h` (copy the existing
block format, signature string included). Calling Java from C: look up by name like
`on_touch_outside` does (`_METHOD(_CLASS(obj), "name", "()V")`, then
`CallVoidMethod` + `ATL_CALLBACK_CHECK(env)`), don't touch the upcall generator.

**A. Dialog semantics (affects every app, Fossify included)** — `Dialog.java`
1. Fields `boolean cancelable = true; boolean canceled; boolean created; boolean
   showing; boolean closeOnTouchOutsideSet; OnCancelListener onCancelListener;`.
   `setCancelable` stores the flag; `setOnCancelListener` stores the listener.
2. `cancel()`: `if (!canceled && onCancelListener != null) { canceled = true;
   onCancelListener.onCancel(this); } dismiss();`
3. `public void onBackPressed() { if (cancelable) cancel(); }` and
   `public boolean onTouchEvent(..)` unchanged. Add `protected void onStart() {}`
   and `protected void onStop() {}`.
4. `setCanceledOnTouchOutside(cancel)`: `if (cancel && !cancelable) cancelable =
   true; closeOnTouchOutsideSet = true;` then the native call. In `show()` (L105-107)
   apply the theme value only `if (!closeOnTouchOutsideSet)` (and call the native
   method directly there so it does not set the flag).
5. `show()`: return early if `showing`; `canceled = false`; call `onCreate(null)`
   only if `!created` then `created = true`; call `onStart()` before `nativeShow`;
   set `showing = true`. Keep the main-looper posting as is.
6. `dismiss()`: keep the 10 ms post (NewPipe hack, comment L145-148), but inside
   the runnable `if (!showing) return; showing = false; nativeClose(..); onStop();`
   then the dismiss listener. `hide()` must not clear `showing`.
7. Add `public void onTouchOutsideFromNative() { if (cancelable && showing) cancel(); }`.

**B. Native cancel paths** — `android_app_Dialog.c`
8. `on_touch_outside` (L26-38): call `onTouchOutsideFromNative` instead of `dismiss`.
9. `on_close_request` (L11-17): call Java `onBackPressed()` and `return TRUE`
   (Java closes the window via `nativeClose` when it dismisses; returning TRUE keeps a
   non-cancelable dialog open).
10. `nativeInit` (L53-61): add `gtk_event_controller_key_new()` on the dialog window
    (bubble phase); in its `key-pressed` handler, for `GDK_KEY_Escape` call Java
    `onBackPressed()` on the dialog object and return TRUE; other keys FALSE. Store
    the jobject with `_REF(this)` (as L59 does). Virtual call, so androidx
    `ComponentDialog.onBackPressed` -> dispatcher -> `Dialog.onBackPressed` works.

**C. Framework AlertDialog: native slots** — keep the GTK layout, add slots
11. `android_app_AlertDialog.c` `nativeConstruct`: build, in this order, a vertical
    GtkBox `title_slot` (empty), the message label, the list view, a vertical GtkBox
    `list_slot` (empty), a vertical GtkBox `custom_slot` (empty, `vexpand` TRUE),
    and a horizontal GtkBox `button_bar` with: neutral button wrapper, a spacer
    (`gtk_box_new` + `gtk_widget_set_hexpand(TRUE)`), negative, positive (replaces
    L60-62). Store the slots with `g_object_set_data(G_OBJECT(dialog), "title_slot",
    ..)` etc. Hide the label while its text is empty (`gtk_widget_set_visible`).
12. New native `nativeSetSlotView(long ptr, String slot, long widget)`: look up the
    slot box by name, remove its existing children
    (`while ((c = gtk_widget_get_first_child(box))) gtk_box_remove(box, c);`), then
    `gtk_box_append(box, gtk_widget_get_parent(GTK_WIDGET(_PTR(widget))))` (Android
    views go in by their wrapper, the parent of `view.widget`, like the buttons at
    L60-62). widget 0 = just clear. Add the header block.
13. `AlertDialog.java`: field `View customView`; `setView(View v)`: `customView = v;
    nativeSetSlotView(nativePtr, "custom", v.widget)`; `setView(int)` inflates with
    `LayoutInflater.from(getContext()).inflate(id, null)`. Override
    `findViewById(int id)`: super first, then `customView.findViewById(id)`, then the
    custom title. Do NOT call `setContentView`. This also fixes Date/TimePickerDialog.
14. `setCustomTitle(View v)` -> slot "title" (keep the GTK window title as is).
15. Builder: `setView(View)`, `setView(int)` -> dialog; `setCancelable` stores a
    `cancelable` field (default true); `setOnCancelListener`, `setOnDismissListener`,
    `setOnKeyListener` (store; Dialog's is a no-op for now) forward to the dialog;
    `create()` and `show()` apply `dialog.setCancelable(cancelable); if (cancelable)
    dialog.setCanceledOnTouchOutside(true);` once. Add `getContext()`,
    `setIcon(Drawable)` (no-op is fine: icon not drawn), `setCustomTitle`.
16. `activate_cb` (L30-38): after `onClick`, dismiss: call Java `dismiss` on `this`
    by name (dismiss is posted, so the click callback runs first, as in AOSP).
    Also make `setMessage` win: if a message is set, hide the list (AOSP order).
17. Choice lists: `setSingleChoiceItems(CharSequence[]|int arrayId, int checkedItem,
    OnClickListener)` and `setMultiChoiceItems(CharSequence[]|int, boolean[],
    OnMultiChoiceClickListener)`. Native `nativeSetChoiceItems(long ptr, String[]
    items, boolean[] checked, boolean multi)`: fill `list_slot` with a vertical GtkBox
    of `gtk_check_button_new_with_label`; for single choice put them in one group
    (`gtk_check_button_set_group(btn, first)`), so they look like radio buttons; on
    `toggled` call Java `onChoiceItemToggled(int which, boolean checked)` (by name;
    for single choice ignore the "unchecked" toggle). Java keeps the `boolean[]`,
    calls `onClick(this, which)` or `onClick(this, which, isChecked)`; never dismisses.
    (Not `android.widget.CheckedTextView`: it is a 20-line stub with no check mark.)
18. `setAdapter(ListAdapter, OnClickListener)`: create `android.widget.ListView`,
    `setAdapter`, `setOnItemClickListener` (anonymous class: `listener.onClick(dialog,
    position); dialog.dismiss();`), put it in slot "list". Add `getListView()`
    returning it (null otherwise).

## Failing checks to write first (docs/agent-tasks/test-first.md)
Interaction syntax: docs/usage.md "Interaction scripts". Screen is 1080x1920; the
dialog is a separate GTK window and appears at the top-left (no window manager).
Coordinates of the dialog's widgets: append `shot dlg` after opening it, run
`scripts/rig -r mini atlh run --run-id dlg0 --pkg org.fossify.notes --force`, then
`scripts/rig pull` and look at `work/.../org.fossify.notes.dlg.png`. Main-screen
coordinates: `scripts/rig -r mini smoke work/apks/<apk> --settle 20` (in
`work/remote/mini/smoke/org.fossify.notes_13/shot.png` the "+" is at ~914,83).

`interactions/org.fossify.notes.txt` (append; the existing steps type into the note):
```
tap 914 83            # "+" = Add a new note -> NewNoteDialog (appcompat path)
wait 2
shot newnote
expect-changed
expect-text "Add a new note"   # custom title
expect-text "Checklist"        # custom view content
expect-text "Cancel"           # button bar
key Escape                     # back: cancelable dialog must close (fails today: no key path)
wait 1
shot afteresc
expect-changed
```
Run on unfixed main twice: the first three expect-text steps tell whether the
appcompat path already renders (likely yes); the Escape step must fail. Note this
makes the Notes script's baseline fail; rebase the gate run after committing it.
For the framework AlertDialog (part C) no Fossify app uses it: find a corpus app
that logs `making an AlertDialog$Builder` (`grep -a -l 'AlertDialog\$Builder as we
speak' work/runs/*/*.log` on a runner, or `atlh missing-apis` for
`AlertDialog$Builder.setSingleChoiceItems`/`setView`) and script its dialog the same
way (expect-text of title, of a custom-view text, and of a button). If none exists,
say so in HANDOFF.md and gate part C on "no regressions" only.

## Done when
- The Notes script passes on mini and in the gate run; the Keyboard startup dialog
  still renders (its screenshot unchanged apart from fixes).
- Fork: `rig build --worktree ~/Develop/android-translation-layer` clean;
  `rig atlh run` (Fossify corpus + the part-C app) and `rig atlh compare <base>
  <new>`: 0 regressed. Run long things with `--detach` + `rig job wait`.
- Harness repo: commit the script; `cargo fmt && cargo clippy --all-targets &&
  cargo test` and `python3 -m unittest discover -s scripts/tests` pass. Update this
  file's status, `docs/agent-tasks/triage.md` rows (setView, setCancelable), HANDOFF.md.

## Risks / what not to do
- Don't route `Builder.setView`/`AlertDialog.setView` through `setContentView`
  (replaces or hides title/message/buttons; trial 3 was rejected for that).
- Don't swallow cancel by catching exceptions or returning early without reason;
  no empty stubs for new Builder methods that drop the app's data.
- Don't change Window.setContentView: the appcompat path depends on it.
- `onCreate` once: dialogs that relied on re-running `onCreate` on re-show (none
  known) would change; appcompat needs once. Keep the 10 ms dismiss delay.
- `on_close_request` returning TRUE: if Java throws before dismissing, the window
  stays; that is correct for `setCancelable(false)`.
- Touch-outside remains "any press on the main window"; positioning/centering and a
  dim background are out of scope (no WM under Xvfb).
- Calling `onStart`/`onStop` changes androidx `ComponentDialog` lifecycle to
  STARTED/CREATED: correct per AOSP, but watch the gate for dialog-heavy apps.
- Nothing goes upstream (AGENTS.md hard rules).
