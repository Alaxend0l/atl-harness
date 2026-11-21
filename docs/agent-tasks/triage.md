<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Triage: local model vs Claude (2026-11-21)

Seven read-only agents went over every audit finding (139) against the fork's
`main` (16b4bb4d) and surveyed the fork for problems the audit missed (27).
Each item got a verdict:

- **local**: suitable for the overnight local model (`scripts/agent-loop`): small
  (median ~12 lines, 1-2 Java files), behaviour unambiguous from AOSP, no C/JNI/
  header/meson change, no design, and a reviewer can judge it from the diff.
  Each carries a precise `fix_hint` the loop puts in the prompt.
- **claude**: native/C, rendering pipeline, binder/IPC, threading, multi-file
  or design work, or anything a plausible-but-wrong fix would pass the gate on.

Why the bar is that strict: the gate only sees startup (plus a few interaction
scripts), so a wrong fix in a dialog or click path passes. The first trial
showed it: of two one-line AlertDialog fixes, one was right and one forwarded
`Builder.setView` to a method that replaces the whole dialog content.

| | audit | survey |
|---|---|---|
| local | 47 | 18 |
| claude | 82 | 9 |
| already fixed | 10 | |
| not a bug | 0 | |

The loop's queue is `local-queue.json` (local items, by user impact then size).
3 local-sized survey items in the media-session/audio path are held back
while Claude works on Fossify Music playback.

## Local queue

| # | impact | ~lines | item | where |
|---|---|---|---|---|
| 1 | high | 2 | AlertDialog button with null listener throws NullPointerException on click | src/api-impl/android/app/AlertDialog.java:43 |
| 2 | high | 3 | setPaddingRelative() is empty, so padding set from code with start/end is dropped | src/api-impl/android/view/View.java:2307 |
| 3 | high | 8 | setImageBitmap and setImageResource (bitmap path) never update the drawable field, so getDrawable() is stale o | src/api-impl/android/widget/ImageView.java:110 |
| 4 | high | 15 | ImageView.setColorFilter(...) overloads are all empty | src/api-impl/android/widget/ImageView.java:213 |
| 5 | high | 20 | Color.colorToHSV and HSVToColor give wrong results (hue offsets and a fraction multiplied by 6) | src/api-impl/android/graphics/Color.java:162 |
| 6 | high | 20 | addOnAttachStateChangeListener keeps only one listener, so each call replaces the previous one | src/api-impl/android/view/View.java:1725 |
| 7 | high | 35 | TextView.setCompoundDrawablesWithIntrinsicBounds / ...Relative... are empty, XML drawableStart/Left/Top are ne | src/api-impl/android/widget/TextView.java:427 |
| 8 | medium | 2 | Context.getColor/getColorStateList resolve without the context theme | src/api-impl/android/content/Context.java:203 |
| 9 | medium | 3 | ArgbEvaluator.getInstance() returns null, so ValueAnimator/ObjectAnimator.ofArgb and XML color animators get n | src/api-impl/android/animation/ArgbEvaluator.java:7 |
| 10 | medium | 3 | Path.addCircle is a logging stub (circular clips, avatars, ring cut-outs draw nothing) | src/api-impl/android/graphics/Path.java |
| 11 | medium | 4 | StateListDrawable inflates items without the theme, so ?attr references in selector items don't resolve | src/api-impl/android/graphics/drawable/StateListDrawable.java:137 |
| 12 | medium | 4 | Theme.applyStyle/setTo never update ThemeKey, so every Theme shares one drawable/ColorStateList cache bucket | src/api-impl/android/content/res/Resources.java:1255 |
| 13 | medium | 6 | ProgressBar/SeekBar ignore android:max and android:progress from XML | src/api-impl/android/widget/ProgressBar.java:72 |
| 14 | medium | 6 | TextPaint(Paint), TextPaint(int flags) and TextPaint.set() drop the source paint even though Paint(Paint)/set( | src/api-impl/android/text/TextPaint.java:52 |
| 15 | medium | 10 | NetworkCapabilities never includes NET_CAPABILITY_VALIDATED, and NetworkInfo.isAvailable() is always false, so | src/api-impl/android/net/ConnectivityManager.java:52 |
| 16 | medium | 10 | Settings.Secure/Global.getString return sentinel strings ("NOTICEME", "STRING_FROM_SETTINGS_GLOBAL_WITH_KEY_.. | src/api-impl/android/provider/Settings.java:28 |
| 17 | medium | 12 | selectDrawable(-1) keeps showing the previous child instead of clearing it | src/api-impl/android/graphics/drawable/DrawableContainer.java:18 |
| 18 | medium | 12 | DrawableContainer.setBounds skips super and never gives newly selected children bounds | src/api-impl/android/graphics/drawable/DrawableContainer.java:113 |
| 19 | medium | 12 | withEndAction/withStartAction are never cleared and re-run on every later animate() call | src/api-impl/android/view/ViewPropertyAnimator.java:160 |
| 20 | medium | 15 | Button.setText(null) shows "null"; Button and EditText don't override setText(CharSequence, BufferType), so th | src/api-impl/android/widget/Button.java:53 |
| 21 | medium | 15 | PropertyValuesHolder ignores the evaluator for int/float values and ignores keyframes, so ofArgb/ArgbEvaluator | src/api-impl/android/animation/PropertyValuesHolder.java:130 |
| 22 | medium | 15 | <include> ignores android:visibility (and other overrides besides id and layout_*) on the include tag | src/api-impl/android/view/LayoutInflater.java:323 |
| 23 | medium | 20 | Intent copy constructor and putExtras alias Bundles instead of copying/merging; copies drop flags/type/package | src/api-impl/android/content/Intent.java:674 |
| 24 | medium | 20 | scrollTo/setScrollX/setScrollY never call onScrollChanged or the scroll-change listener (setOnScrollChangeList | src/api-impl/android/view/View.java:1643 |
| 25 | medium | 30 | Window.addFlags/setFlags/clearFlags are empty: FLAG_KEEP_SCREEN_ON, FLAG_FULLSCREEN and others are ignored and | src/api-impl/android/view/Window.java:56 |
| 26 | medium | 35 | AudioManager reports ringer mode SILENT and stream volume 0 | src/api-impl/android/media/AudioManager.java |
| 27 | medium | 35 | AccountManager.addOnAccountsUpdatedListener(listener, handler, boolean) and android.content.SyncStatusObserver | src/api-impl/android/accounts/AccountManager.java, src/api-i |
| 28 | medium | 45 | Framework PopupMenu Menu methods throw UnsupportedOperationException (add(CharSequence), add(int,int,int,int), | src/api-impl/android/widget/PopupMenu.java |
| 29 | medium | 50 | Vibrator has no vibrate(VibrationEffect); VibrationEffect drops its data; repeat index never loops | src/api-impl/android/os/Vibrator.java, src/api-impl/android/ |
| 30 | medium | 60 | ContentResolver.registerContentObserver/notifyChange are no-ops, so cursors and observers never refresh | src/api-impl/android/content/ContentResolver.java, src/api-i |
| 31 | medium | 60 | Framework PopupMenu MenuItem methods throw UnsupportedOperationException (getTitle, isChecked, isEnabled, getG | src/api-impl/android/widget/PopupMenu.java |
| 32 | low | 1 | ProgressBar.setIndeterminate assigns the parameter instead of the field, so isIndeterminate() never changes | src/api-impl/android/widget/ProgressBar.java:61 |
| 33 | low | 1 | Editor.putX(key, null) stores null instead of removing the key | src/api-impl/android/app/SharedPreferencesImpl.java:423 |
| 34 | low | 1 | CountDownTimer delays the first onTick by one interval | src/api-impl/android/os/CountDownTimer.java:24 |
| 35 | low | 1 | Window.peekDecorView() returns null although the decor view always exists | src/api-impl/android/view/Window.java:96 |
| 36 | low | 2 | Activity.moveTaskToBack and setFinishOnTouchOutside are package-private | src/api-impl/android/app/Activity.java:701 |
| 37 | low | 3 | Canvas.drawText(char[], index, count, ...) treats count as an end index | src/api-impl/android/graphics/Canvas.java:188 |
| 38 | low | 3 | IntentFilter.match discards the action-mismatch result | src/api-impl/android/content/IntentFilter.java:81 |
| 39 | low | 3 | AccelerateDecelerateInterpolator (the default for every ValueAnimator) and PathInterpolator are linear | src/api-impl/android/view/animation/AccelerateDecelerateInterpolator.java:1 |
| 40 | low | 3 | attachViewToParent adds a view twice when it was not previously detached | src/api-impl/android/view/ViewGroup.java:269 |
| 41 | low | 4 | Drawable caches' shouldInvalidateEntry throws, so Resources.updateConfiguration crashes after drawables are ca | src/api-impl/android/content/res/Resources.java:113 |
| 42 | low | 5 | ViewGroup.offsetChildrenTopAndBottom is an empty FIXME, but GridView depends on it | src/api-impl/android/view/ViewGroup.java:622 |
| 43 | low | 5 | Debug System.out.println in hot paths (Drawable.updateTintFilter prints on every tint update) | src/api-impl/android/graphics/drawable/Drawable.java (and si |
| 44 | low | 6 | TextView.setHint is empty and getHint() returns the literal "HINT" | src/api-impl/android/widget/TextView.java:374 |
| 45 | low | 6 | openFileOutput ignores MODE_APPEND; fileList() always empty | src/api-impl/android/content/Context.java:396 |
| 46 | low | 6 | getSuggestedMinimumWidth/Height ignore the background drawable's minimum size | src/api-impl/android/view/View.java:1812 |
| 47 | low | 8 | DateFormat.getDateFormat returns a date+time SimpleDateFormat; getDateFormatString is hard-coded to "dd. MM. y | src/api-impl/android/text/format/DateFormat.java:211 |
| 48 | low | 8 | unscheduleDrawable is a no-op, so animated drawables cannot be stopped | src/api-impl/android/view/View.java:1564 |
| 49 | low | 10 | getClipToPadding()/getClipChildren() return false; AOSP defaults are true and the setters are no-ops | src/api-impl/android/view/ViewGroup.java:636 |
| 50 | low | 12 | VectorDrawable ignores android:fillType, so evenOdd paths are filled with the nonzero rule | src/api-impl/android/graphics/drawable/VectorDrawable.java:2195 |
| 51 | low | 15 | ThumbnailUtils.extractThumbnail stretches instead of centre-cropping | src/api-impl/android/media/ThumbnailUtils.java |
| 52 | low | 20 | Ringtone.getTitle returns a visible placeholder string; RingtoneManager.getDefaultUri returns an empty Uri | src/api-impl/android/media/Ringtone.java:7 |
| 53 | low | 20 | Context.deleteFile(String) and android.util.LogPrinter missing | src/api-impl/android/content/Context.java, src/api-impl/andr |
| 54 | low | 25 | InputFilter.LengthFilter.filter always returns "" (delete) instead of null (accept), and has no max | src/api-impl/android/text/InputFilter.java:11 |
| 55 | low | 25 | android.view.animation.BounceInterpolator missing (ClassNotFoundException crash) | src/api-impl/android/view/animation/BounceInterpolator.java  |
| 56 | low | 30 | LayerDrawable.setColorFilter is commented out and setTintList is missing, so layer-list and ripple tints are i | src/api-impl/android/graphics/drawable/LayerDrawable.java:431 |
| 57 | low | 30 | Canvas.drawPoint, drawARGB and drawColor(int, PorterDuff.Mode) are logging stubs; drawPoints treats offset/cou | src/api-impl/android/graphics/Canvas.java |
| 58 | low | 30 | Toast: no setText/setDuration/getDuration, cancel() does nothing, duration ignored | src/api-impl/android/widget/Toast.java |
| 59 | low | 35 | WebViewClient callbacks use non-AOSP signatures: onPageStarted(WebView,String) and only the String shouldOverr | src/api-impl/android/webkit/WebViewClient.java:4 |
| 60 | low | 35 | View.dispatchApplyWindowInsets missing, setOnApplyWindowInsetsListener is a no-op and its interface has no met | src/api-impl/android/view/View.java, src/api-impl/android/vi |
| 61 | low | 40 | TextUtils.htmlEncode returns its input unescaped; TextUtils.ellipsize never truncates | src/api-impl/android/text/TextUtils.java:354 |
| 62 | low | 70 | NotificationChannel stores nothing and has no getters; getNotificationChannel(s) return null/empty; cancelAll  | src/api-impl/android/app/NotificationChannel.java, src/api-i |

## Held back (media session / audio, in progress)

- MediaSession.setPlaybackState throws NPE when the active queue item has no title or subtitle
- MediaMetadata is an empty shell: Builder drops every key and there are no getters
- PlaybackState has no getters or STATE_/ACTION_ constants and Builder(from) aliases; MediaDescription has no getters

## For Claude

| impact | ~lines | item | why |
|---|---|---|---|
| high | 60 | AlertDialog.Builder.setView(View) drops the custom view | The Builder.setView no-op is real. A correct fix has to put the custom view into a slot inside the native AlertDialog layout, between the message/items and the  [repro: interactions/org.fossify.musicplayer.txt fails at ExpectText("TestList") on main 7cb4cd2b, mini runs setv1-setv4; see docs/agent-tasks/test-first.md] |
| high | 150 | AnimationUtils.loadInterpolator returns null: NPE crash in Fossify File Manager and Music Player (Ma | This is the root cause of the roadmap's 'PropertyValuesHolder.setAnimatedValue NPE' family. In logs fossify-sp2 filemanager and musicplayer, l43.set calls TimeI |
| high | 30 | AssetManager.setConfiguration drops the locale, so localized resources are never selected | Confirmed: setConfiguration takes a locale jstring but never fills ResTable_config language/country. Parsing a BCP-47 or legacy locale into the packed config fi |
| high | 25 | Bitmap.getPixel()/partial getPixels() exits the whole process | Confirmed: native_get_pixels calls exit(1) on any partial read, and getPixel always does a partial read. Implementing a sub-rect download (download the full tex |
| high | 200 | DateUtils relative/formatted time methods return Date.toString() regardless of flags | Implementing getRelativeTimeSpanString, formatDateTime and the rest with flag semantics (FORMAT_SHOW_DATE/TIME/ABBREV, relative resolutions, locale patterns, IC |
| high | 50 | Dialogs always close on window close or outside tap: setCancelable is a no-op, cancel() never fires  | Confirmed: setCancelable and setOnCancelListener are empty, cancel() just dismisses, and the C close-request and outside-tap handlers always dismiss. A fix need |
| high | 50 | Harness: agent-loop takes audit items by severity, including native/rendering ones the local model c | Harness repo, outside the agent's scope. The queue (AUDIT json, high first) feeds the small model C/GSK and Parcel items it cannot build or verify, which wastes |
| high | 400 | Harness: behaviour-probe APK in the gate so semantic framework fixes are actually checked | The gate only sees startup, so local-model fixes like the AlertDialog.setView mistake pass unchecked. A small APK that calls framework APIs and logs PROBE PASS/ |
| high | 200 | Html.fromHtml returns the raw HTML string, which TextView then feeds to Pango markup; plain Spanned  | A real fix needs an HTML-to-spans parser, plus a change to how TextView picks Pango markup or plain text (native_set_markup is never reset). That is cross-file, |
| high | 60 | Intent.resolveActivity returns null for implicit intents ATL can actually handle (VIEW URL, SEND) | A correct resolveActivity needs a policy for which implicit intents ATL can serve externally (xdg-open for VIEW http/tel/mailto, SEND, chooser), and that policy |
| high | 150 | MediaPlayer stop/pause/release/seekTo/setLooping/setVolume are empty, isPlaying is always false, cre | stop, pause, seek, loop, volume, isPlaying and create(ctx,resId) need new native methods on the GtkMediaStream in C/JNI, regenerated headers, and a resource-to- |
| high | 200 | Paint shaders are stored in Java only and never drawn, so <shape><gradient> renders solid black | Shaders never reach the native paint. The fix needs native gradient support (GskCanvas/Paint C, gsk gradient nodes) and new JNI plumbing for LinearGradient/Radi |
| high | 30 | TextView text size is applied in points, while Paint uses pixels; getTextSize() returns a fixed 10 | Confirmed: TextView C applies the size with pango_attr_size_new (points) while Paint uses absolute pixels, getTextSize() returns 10, and units are ignored. A re |
| high | 30 | canScrollVertically/canScrollHorizontally always return false instead of using computeVertical/Horiz | The AOSP formula is short, but it depends on computeVerticalScrollOffset (here -1, not scrollY) and on how each widget (native-backed ScrollView, AbsListView, R |
| high | 300 | createFromXmlInner returns null or an empty drawable for many standard XML tags | Covers many drawable types (clip, scale, rotate, level-list, color, transition, nine-patch, animation-list, animated-*), each needing working inflate and draw l |
| high | 80 | maxLines/singleLine/lines/ellipsize are ignored from XML and code; the label always wraps freely | Needs native GtkLabel work (wrap, lines, ellipsize mode via JNI and new natives with header regeneration), plus XML reading. Getting the layout and measure inte |
| high | 80 | requestPermissions never delivers onRequestPermissionsResult; checkPermission denies all unlisted (i | Making requestPermissions work needs design decisions. Someone has to decide which permissions ATL grants, deliver onRequestPermissionsResult asynchronously on  |
| high | 50 | setBackgroundTintList/Mode are empty and getBackgroundTintList returns null; the XML backgroundTint  | Real backgroundTint support needs a state-dependent ColorStateList kept in sync with drawable state, the XML attribute read as a list, and handling of the nativ |
| medium | 200 | <propertyValuesHolder>/<keyframe> children in animator XML are parsed and discarded (loadPvh/getPVH  | Restoring loadPvh and getPVH needs keyframe parsing and keyframe support in PropertyValuesHolder.calculateValue, which currently ignores keyframes. That is mult |
| medium | 50 | AlarmManager.cancel is a no-op, re-setting an alarm adds duplicates, setAlarmClock does nothing | Making cancel and re-set work needs a registry of scheduled runnables keyed by PendingIntent equality (AOSP uses Intent.filterEquals plus the request code), and |
| medium | 80 | AnimationDrawable.inflate is commented out, and the oneShot default is true instead of false | Re-enabling inflate depends on missing helpers (inflateWithAttributes, setVariablePadding), on createFromXmlInner calling inflate, and on frame scheduling worki |
| medium | 60 | Application.registerActivityLifecycleCallbacks is an empty no-op | Storing the callbacks is trivial, but they only help if every Activity lifecycle transition (create, start, resume, pause, stop, save state, destroy) dispatches |
| medium | 70 | AudioTrack.stop()/flush() are logging stubs and getTimestamp() is always false: media3 seek and stop | Native ALSA work (snd_pcm_drop/prepare), playback-head bookkeeping and play-state transitions that media3's AudioTrackPositionTracker relies on. Getting this wr |
| medium | 60 | Bitmap.compress(JPEG/WEBP) writes the text 'fixme Bitmap.compress JPEG' into the output stream | JPEG/WEBP encoding needs a native encoder (gdk-pixbuf or libjpeg via JNI). A Java-only change could only make it fail cleanly. |
| medium | 3 | Bitmap.setPixels/setPixel read ARGB ints as R8G8B8A8, swapping red and blue | Confirmed: native_set_pixels uses GDK_MEMORY_R8G8B8A8 for little-endian ARGB ints. It should be B8G8R8A8 (unpremultiplied). The fix is in C, and the premultipli |
| medium | 200 | BitmapDrawable ignores alpha, tint and colour filter, getPaint() returns a throwaway Paint, and <bit | Alpha, tint and colour filter for bitmaps need native paintable/texture rendering support, and the <bitmap> attributes (gravity, tileMode) need layout and tilin |
| medium | 70 | BitmapFactory ignores inSampleSize, and inJustDecodeBounds decodes the whole image | Needs a native size-only probe (gdk_pixbuf_get_file_info or loader size-prepared) and sampled decoding. outWidth/outHeight semantics are subtle: AOSP applies th |
| medium | 50 | Button/EditText/CompoundButton override setTextSize (and CompoundButton setTextColor) with empty bod | TextView.setTextSize/setTextColor are native and work on the label inside TextView's box. Button (GtkButton), EditText (GtkText) and CompoundButton need their o |
| medium | 30 | Canvas.drawColor(int, PorterDuff.Mode) is a logging stub | Glide's bitmap transforms call it (drawColor(0x00000000, CLEAR)); it became visible as a W/Canvas signature in every Fossify Music/Gallery run after the View.layout() layout-listener fix (fork 7cb4cd2b) made transforms run. Needs a native blend/erase: CLEAR -> clear the snapshot, SRC_OVER -> drawPaint, others via gtk_snapshot_push_blend. |
| medium | 6 | Canvas.drawLines steps by 1 float instead of 4, ignores offset+count and reads past the array | Confirmed: the native_drawLines loop steps by 1 float, compares i to count rather than offset+count, and reads past the array. The fix is in C JNI (step by 4, b |
| medium | 70 | Canvas.saveLayer ignores the layer paint (alpha, xfermode) and saveLayerAlpha is missing (NoSuchMeth | Needs a GSK opacity, blend or mask node pushed on save and popped on restore, tracked in the save stack. Rendering-pipeline work in native code, and a Java-only |
| medium | 300 | Character spans (ForegroundColor, BackgroundColor, RelativeSize, AbsoluteSize, Typeface, TextAppeara | Rendering spans means changing the CharacterStyle/updateDrawState design and converting spans into Pango attributes in native text rendering for both TextView a |
| medium | 60 | CheckedTextView does not implement Checkable; setChecked is empty and no checkmark is drawn | The Checkable state part is easy, but drawing the checkmark needs native or rendering work, and ListView choice-mode interaction needs runtime checks. A state-o |
| medium | 60 | ColorDrawable has no setAlpha/getAlpha, and setColor neither invalidates nor reaches a View backgrou | View.setBackgroundDrawable copies ColorDrawable's colour into a native background once with no callback. A correct fix changes how View and native code handle C |
| medium | 30 | Configuration.orientation and smallestScreenWidthDp are never set; screenWidthDp uses monitor size,  | Configuration orientation and smallestScreenWidthDp are never set, and the dp values use the monitor's pixel size rather than the window's. A fix needs C geomet |
| medium | 50 | Configuration.setLocales commented out, getLocales ignores the config, default locale hard-coded en_ | Real locale support means restoring LocaleList handling in Configuration, deciding where the default locale comes from (host environment), and propagating it to |
| medium | 20 | Dialog ignores themeResId; getContext() is not wrapped in the dialog theme | Wrapping the dialog context in a ContextThemeWrapper, and resolving android:dialogTheme when themeResId is 0, changes the theme of every dialog and the windowIs |
| medium | 40 | Dialog setCancelable/setOnCancelListener ignored; cancel() never calls OnCancelListener; onCreate re | Partly fixable in Java, by storing the OnCancelListener and creating only once, but native close paths (Escape, touch outside, close button) call dismiss() rath |
| medium | 120 | Drawable.setLevel/getLevel are final no-ops, so level-based drawables never change | Enabling levels means adding Drawable.onLevelChange plus state, and wakes up overrides that have never run or are commented out (ClipDrawable, LevelListDrawable |
| medium | 40 | Focus queries return constants: hasFocus() is false, findFocus() returns this, clearFocus() is a no- | Correct focus answers need a ViewGroup descendant search based on native focus, and clearFocus needs native work. Making findFocus() return null where it now re |
| medium | 60 | Focus-change and key listeners are dropped: setOnFocusChangeListener/setOnKeyListener are empty, OnF | Dispatching focus changes needs native GTK focus hooks plus adding onFocusChange to the interface. Proper key dispatch (OnKeyListener, onKeyUp) also touches the |
| medium | 120 | GradientDrawable reduces solid and stroke ColorStateLists to one colour and never reacts to state; s | Needs ColorStateList storage for solid and stroke, isStateful/onStateChange, tint state updates and SweepGradient (which depends on the missing shader support). |
| medium | 30 | Harness: classifier attaches ART thread-dump frames after a JNI abort to the last exception, produci | This is in the atl-harness repo, which the local agent cannot edit. Example: fossify-sp2 filemanager gets 'NullPointerException @ android.atl.ATLMediaContentPro |
| medium | 100 | HorizontalScrollView is a plain FrameLayout with no native scroller | Needs a new native scroller widget (C, JNI, generated header, meson) and design work. |
| medium | 80 | ImageView scale types FIT_CENTER/FIT_START/FIT_END/MATRIX are ignored and CENTER scales down | ImageView scale types map onto GtkPicture content-fit in C. FIT_START/FIT_END, CENTER without scaling and MATRIX need custom layout or snapshot logic in C, whic |
| medium | 60 | Inflater passes the inflater's root context to factories and children, so android:theme overlays do  | For theme overlays to cascade, the context has to be threaded through createViewFromTag, rInflate, parseInclude and the factory calls, wrapping it in ContextThe |
| medium | 120 | InsetDrawable ignores insets from both the constructors and XML | The fix means porting InsetState fields, XML inset/fraction parsing, onBoundsChange shrinking, getPadding, intrinsic size and newDrawable from AOSP. That is wel |
| medium | 120 | LayerDrawable ignores the item gravity, width, height, start and end attributes and their setters | Needs AOSP's layer bounds computation (Gravity.apply with width/height, start/end resolved for layout direction, padding modes) inside onBoundsChange. Substanti |
| medium | 25 | Lifecycle: onRestart never called; onPostCreate runs before onStart | Confirmed: onRestart is never called and onPostCreate runs before onStart. Reordering the activity lifecycle in C (activity_focus) is threading and lifecycle wo |
| medium | 6 | Matrix(Matrix src) copy constructor ignores src and returns identity | Confirmed: native_create ignores src and always returns identity. The fix is in C (graphene_matrix_init_from_matrix when src != 0). Tiny, but it is native code. |
| medium | 20 | Matrix.setRotate(deg) adds to the existing rotation instead of resetting; preScale/preRotate with a  | Confirmed: setRotate(deg) calls graphene_matrix_rotate_z on the existing matrix without resetting it, and the pivot pre-ops have suspect order. Fixing it means  |
| medium | 80 | MediaPlayer.setDataSource(Context, Uri) passes only uri.getPath() to a file-only GTK stream; http/co | Supporting content://, http:// and FileDescriptor sources needs native changes (gtk_media_file_new_for_file with GFile/GInputStream, or reading through ContentR |
| medium | 80 | MediaSession.setMetadata is a no-op: MPRIS and lock screen show no title, artist, art or length for  | Needs the stored MediaMetadata (the item above) wired into MPRIS: title, artist, art URL or bitmap-to-file, and length via media_player2_player_set_metadata. Th |
| medium | 120 | Paint colour filter ignores the PorterDuff mode, and other ColorFilter types are dropped | Native C colour-filter change: implement the PorterDuff modes and the ColorMatrix/Lighting filters in android_graphics_Paint.c, plus the Java-side matrix plumbi |
| medium | 150 | Paint text measurement returns guesses and constants although native Pango metrics exist | Replacing the guesses with real Pango metrics means wiring native measurement (JNI, maybe new native methods and header regeneration) across several Paint metho |
| medium | 15 | Path.arcTo/addArc with a 360-degree sweep draws nothing | Confirmed: for a 360-degree sweep the SVG arc endpoints p1 and p2 coincide, so nothing is drawn. A fix splits the arc into two segments in C path-building code. |
| medium | 250 | PreferenceManager.setDefaultValues does not write XML defaults; PreferenceActivity.addPreferencesFro | Writing XML defaults needs preference-XML inflation (a PreferenceInflater, reading android:defaultValue per Preference type), and a real findPreference needs a  |
| medium | 80 | ScrollView: fillViewport ignored, fullScroll returns true without scrolling, scrollTo/smoothScrollTo | Needs native GtkScrolledWindow adjustment setters/getters, a View.scrollTo integration and fillViewport layout behaviour, across several files plus JNI. |
| medium | 40 | SeekBar fires start, progress and stop tracking on every drag step; programmatic setProgress never n | Confirmed: on_change_value fires start, progress and stop on every step, and programmatic setProgress never notifies. A fix needs GTK gesture begin/end tracking |
| medium | 50 | SeekBar listener fires start/changed/stop on every change-value, before the value is applied, and ne | C/GTK signal-handling rework: track drag begin/end via gestures, fire onProgressChanged after the value is applied (value-changed), and handle programmatic chan |
| medium | 100 | StateListDrawable instances from the Resources cache share child drawables, and the container passes | Fixing shared children needs a proper ConstantState/mutate design (newDrawable must create fresh child drawables from their constant states) plus alpha/tint/fil |
| medium | 150 | StaticLayout ignores alignment, start/end range, spacing, maxLines and ellipsize; Builder setters ar | Honouring alignment, range, spacing, maxLines and ellipsize needs new native Layout.c Pango calls, header regeneration, and a Builder redesign so the layout is  |
| medium | 60 | TextView typeface is ignored: setTypeface(Typeface) is empty, setTypeface(tf, style) ignores tf, and | Applying a Typeface needs native font-family/Pango or CSS plumbing and a design for mapping Typeface objects (including asset fonts). Not a Java-only change. |
| medium | 60 | TextWatcher callbacks are wrong: beforeTextChanged never fires, and onTextChanged gets start=0,befor | Native C rework: hook GtkEditable insert-text/delete-text to compute character (not byte) offsets and fire beforeTextChanged and onTextChanged correctly. Easy t |
| medium | 50 | TextWatcher callbacks get UTF-8 byte counts, before=0, a detached copy, and beforeTextChanged never  | TextWatcher arguments come from GTK changed signals in C, and the counts are UTF-8 byte counts. Getting correct start/before/count and a beforeTextChanged call  |
| medium | 200 | Typeface is fake: DEFAULT_BOLD is not bold, custom fonts load as DEFAULT, and Paint.setTypeface is a | A real Typeface needs a native font-family/weight representation, asset font loading and Paint/Pango integration. This is design work involving C. |
| medium | 60 | Unmapped GDK keyvals are passed through as Android keycodes; shifted symbols collide with letter key | Confirmed: map_key_code's default returns the raw GDK keyval as an Android keycode. A proper mapping table plus unicode-char handling for shifted symbols is C i |
| medium | 300 | View animations (Alpha/Translate/Rotate/Scale/AnimationSet, AnimationUtils.loadAnimation) never touc | Implementing the view Animation framework (Alpha/Translate/Scale/Rotate, AnimationSet, XML loading in AnimationUtils) needs the transformations applied to rende |
| medium | 150 | View.setRotation/RotationX/Y, setScaleX/Y and setPivotX/Y are empty and the getters return fixed ide | Rotation, scale and pivot have to reach the GTK render transform. That is native/rendering work: setTranslationX goes through native_queueAllocate, and getMatri |
| medium | 250 | android.app.DialogFragment missing (framework-fragment apps such as Mastodon and its forks go BLANK) | It is a ~300-line AOSP class that depends on ATL's partial Fragment/FragmentManager lifecycle and on Dialog show/dismiss and cancel semantics (audit H14/M5). Th |
| medium | 120 | android:interpolator on XML <animator>/<objectAnimator> is read and then thrown away (setInterpolato | Uncommenting the block alone won't work: AnimationUtils.loadInterpolator(Context,int) is a stub that returns null, and the (res, theme, id) overload doesn't exi |
| medium | 40 | drawBitmap ignores Paint alpha (and the color filter only knows one mode) | drawBitmap ignores Paint alpha, and the colour filter treats every PorterDuff mode as SRC_IN. A fix touches the C rendering pipeline (push_opacity, per-mode col |
| medium | 30 | isLaidOut() always returns true and isShown() always returns true | isShown needs a correct root-of-hierarchy condition in ATL, where the decor parent may be null, or every view reports not shown. isLaidOut needs layout-state tr |
| medium | 80 | registerDefaultNetworkCallback is a no-op; registerNetworkCallback passes a null Network, never fire | Needs C/JNI work: pass a real Network object to callbacks, fire an initial onAvailable, store signal handler ids so unregister can disconnect them, and implemen |
| medium | 40 | sendBroadcast delivers synchronously on the caller's thread; receivers keyed by IntentFilter instanc | Making delivery asynchronous on the main looper, and fixing the receiver bookkeeping (several filters per receiver, receivers registered with a Handler, interac |
| medium | 60 | setBackgroundDrawable ignores the drawable's padding and state, and ColorDrawable backgrounds skip c | AOSP background padding needs user-padding tracking (mUserPadding*), plus drawable state and tint, and the ColorDrawable case takes a separate native fast path. |
| medium | 30 | setResult delivers onActivityResult immediately and repeatedly; finishing without setResult never de | Getting setResult/finish right means storing the result and delivering it once, asynchronously, when the activity finishes, with RESULT_CANCELED as the default. |
| low | 60 | AudioTrack ignores the requested encoding (always S16_LE), write(ByteBuffer) requires an array-backe | Encoding support means changing the native ALSA hw_params format selection (C/JNI). Direct-ByteBuffer writes and real state tracking also touch native code. |
| low | 100 | Canvas clip operations ignore Region.Op, and getClipBounds returns the screen size | Region.Op support (DIFFERENCE etc.) and a real getClipBounds need clip-state tracking in GskCanvas and native snapshot clip handling. |
| low | 40 | Every view is clipped to its own bounds; clipChildren=false/clipToPadding are no-ops | WrapperWidget always clips each view to its own bounds. Supporting clipChildren and clipToPadding means moving clip decisions to the parent in the C snapshot pa |
| low | 60 | GskCanvas.drawRect drops any rectangle whose Paint has an Xfermode, including SRC_OVER and SRC | Real Xfermode support needs native compositing (gtk_snapshot_push_blend/composite) in the GskCanvas C code. Even a partial change (drawing when the mode is SRC_ |
| low | 3 | ImageButton without a src shows GTK's 'image-missing' icon | The fix is in C (android_widget_ImageButton.c: create an empty GtkPicture instead of loading the image-missing resource). It is small, but it is a native change |
| low | 25 | Intent.addCategory ignored; getCategories always empty; getDataString returns "" instead of null | Categories could be added in Java, but getCategories feeds IntentFilter.match, hasCategory and possibly Parcel serialization. AOSP returns null when there are n |
| low | 150 | ObjectAnimator.ofFloat(target, x, y, Path) returns an empty animator with no target | Needs PathKeyframes/KeyframeSet.ofPath (Path approximation, which uses the native graphics path) and keyframe support in PropertyValuesHolder, which doesn't exi |
| low | 60 | Paint Java/native mismatches and wrong defaults: text align, letter spacing, text scale, PathEffect, | A mix of Java/native mismatches: native_clone does not copy Java-side fields, alignment defaults differ between Java and C, and PathEffect/scale are unimplement |
| low | 60 | Path.Direction is ignored, so CW/CCW sub-paths can't cut holes (GradientDrawable ring draws a filled | Path direction has to reach the native GskPathBuilder (contour winding for rect, oval and round rect). That is a C/JNI change. |
| low | 50 | Service.stopSelf does nothing; IntentService runs onHandleIntent on the main thread | stopSelf needs to drive the service lifecycle, including onDestroy and native/service-manager bookkeeping. IntentService needs a worker HandlerThread that proce |
| low | 120 | WebView.getSettings() returns a fresh, stateless WebSettings; evaluateJavascript never calls back; u | This bundles a stateful WebSettings that has to be applied to WebKitGTK (native), and evaluateJavascript needs a native webkit_web_view_evaluate_javascript call |
| low | 150 | android.transition: listeners are dropped (onTransitionEnd never fires) and TransitionInflater.infla | Firing listeners needs TransitionListener's real methods plus a TransitionManager or scene mechanism to call them at the right times, and inflateTransition need |
| low | 30 | getGlobalVisibleRect returns true and the unclipped rect even for views scrolled off screen | getGlobalVisibleRect needs to intersect the rect with ancestor clips and the window by walking GTK widget bounds in C, and the result needs runtime checks. |
| low | 10 | getScaledVerticalScrollFactor()/HorizontalScrollFactor return 1 pixel per wheel unit | The right factor depends on how the native layer turns GTK wheel events into MotionEvents, if it does at all (nothing in the Java/C sources maps AXIS_VSCROLL).  |
| low | 60 | text.Layout mixes UTF-8 byte indices with UTF-16 char offsets | text.Layout mixes Pango UTF-8 byte indices with UTF-16 offsets. A correct fix needs conversion helpers throughout the C Layout glue, and off-by-unit mistakes ar |
