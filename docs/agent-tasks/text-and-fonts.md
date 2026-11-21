<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: real text measurement, fonts and line limits (Paint, Typeface, TextView)

Paths below are relative to the ATL fork (`~/Develop/android-translation-layer`)
unless they start with `docs/` or `interactions/` (those are in the harness).
Triage rows: docs/agent-tasks/triage.md lines 120 (text size), 123
(maxLines/ellipsize), 161 (Paint measurement), 169 (TextView typeface), 172
(Typeface is fake), 191 (Paint defaults).

## Symptom / why it matters
- **TextView text is about 1.33x too big.** The size is applied in points,
  not pixels, so labels overflow, wrap early and push layouts apart. This
  affects every app (Fossify Calculator buttons, list rows, toolbars).
- **Text never truncates.** `maxLines`, `singleLine`, `lines` and
  `ellipsize` are ignored, so one-line list rows (file names, contacts,
  message previews) wrap to several lines and overlap the next row. There is
  never a "…".
- **Custom-drawn text is misplaced.** Views that draw with `Canvas.drawText`
  and centre text using `Paint.measureText`, `getTextBounds` or
  `ascent()/descent()` get guessed numbers: `measureText` returns
  `0.6 * size * chars`, `descent()` returns 0 and `breakText` returns 10.
  You see off-centre digits (calendar month grids, clock faces, charts,
  badges), clipped text and overlapping labels.
- **Custom fonts are ignored.** `Typeface.createFromAsset`,
  `ResourcesCompat.getFont` / `app:fontFamily` and `DEFAULT_BOLD` all give
  the default font, and `Paint.setTypeface` does nothing. Icon fonts
  (Font Awesome, Material icon fonts drawn as text) show as letters or tofu.
- To find corpus apps that ship fonts, run on a runner:
  `rig sh 'for a in work/apks/*.apk; do unzip -l "$a" | grep -qE "res/font/|\.(ttf|otf)$" && echo "$a"; done'`.

## Current code (what is wrong)
`src/api-impl/android/graphics/Paint.java`
- 24, 415: `align` defaults to `Align.CENTER`, but the native default is 0,
  which is `PANGO_ALIGN_LEFT` (`AndroidPaint.h:16`, zeroed by `g_new0`).
  Android's default is LEFT, so `getTextAlign()` lies.
- 83-85 `setTypeface` is a no-op. 320-322 `getTypeface` returns `new Typeface()`.
- 94-98 `getTextWidths` never fills `widths[]` and returns a pixel width
  instead of a count. The other overloads are missing.
- 110-112 `ascent()` returns `-getTextSize()`. 238 `descent()` returns 0.
- 114 `measureText(char[],int,int)` returns 10. 115-117 `measureText(String,int,int)` returns `(end-start)*size*.6`.
- 341-343 all three `breakText` return 10. 396 `hasGlyph` returns false.
- `getFontSpacing`, `getRunAdvance` and `getOffsetForAdvance` are missing
  (NoSuchMethodError).

`src/api-impl-jni/graphics/android_graphics_Paint.c`
- 171-191 `native_get_text_bounds` uses the **logical** rect
  (`pango_layout_get_pixel_extents(layout, NULL, &rect)`, line 180) instead of
  the ink rect, and shifts by the text alignment (182-185). Android's
  `getTextBounds` ignores alignment.
- 199-235 font metrics are real Pango metrics (`pango_context_get_metrics`).
  `top`/`bottom` are set equal to `ascent`/`descent`. That is acceptable and
  can stay.
- 142-146 text size is `pango_font_description_set_absolute_size` (pixels). Correct.
- All of the above call `GetStringUTFChars` (176). That returns *modified*
  UTF-8, where emoji and other non-BMP characters become 6-byte surrogate
  sequences that Pango treats as invalid. The same applies to
  `android_atl_GskCanvas.c:160`.

`src/api-impl-jni/graphics/android_atl_GskCanvas.c:154-183` `native_drawText`: one
PangoLayout, `y -= baseline` (165), and alignment taken from the logical
width (166-169). This is correct in principle. The measurement code must use
the same layout setup so that measuring and drawing agree.

`src/api-impl/android/graphics/Canvas.java:188-190`
`drawText(char[] text, int start, int end, …)`. Android's signature is
`(char[] text, int index, int count, …)`, but this code treats `count` as `end`.

`src/api-impl/android/graphics/Typeface.java` (91 lines): every constant is
the same empty object (11-29). `createFromAsset` and `createFromFile`
(39-62) return DEFAULT. `getStyle()` returns 0. `Builder.build()` returns
DEFAULT. `FontFamily.java` (22 lines) is all no-ops.

`src/api-impl/android/widget/TextView.java` with
`src/api-impl-jni/widgets/android_widget_TextView.c`: a TextView is a GtkBox
containing a GtkLabel (C 18-41, `gtk_label_set_wrap(TRUE)` at 27).
- C 91-107 `setTextSize` uses `pango_attr_size_new(size * PANGO_SCALE)`,
  which is **points**. XML passes pixels (Java 78, 98:
  `getDimensionPixelSize`). Java 156-160 ignores the unit. 329
  `getTextSize()` returns 10.
- 201-219 `setTypeface(tf, style)` only toggles the CSS classes
  `ATL-font-bold`/`ATL-font-italic` (`src/main-executable/css/default-stylesheet.css:7-13`)
  and ignores `tf`. 220 `setTypeface(tf)` is empty. 327 `getTypeface()`
  returns null.
- 221 `setLineSpacing`, 306/481 `setSingleLine`, 318 `setEllipsize`,
  322 `setMaxLines` and 540 `setLines` are all empty. 432 `getMaxLines` returns -1,
  438 `getLineCount` returns 1, 538 `getEllipsize` returns null.
  The constructor (62-116) reads none of maxLines, lines, singleLine,
  ellipsize, typeface or fontFamily.
- Button, EditText, CheckBox, RadioButton and CompoundButton extend TextView
  but build **other** native widgets. Each overrides `setTextSize(float)`
  with `{}` (for example `Button.java:60`). Any new TextView native must check
  that the widget is the box+label pair, as `native_setAlignment` does at
  C 47-53.

`src/api-impl-jni/text/android_text_Layout.c` (StaticLayout/BoringLayout) is
a separate PangoLayout path. It is out of scope here, but keep it consistent
if you add a shared helper.

## What Android does (the contract to implement)
- Units are pixels everywhere in Paint. Y grows downward and the baseline is y=0.
  `FontMetrics`: `ascent` and `top` are **negative** (above the baseline),
  `descent` and `bottom` are positive. `leading` is the extra gap between
  lines (0 for Roboto). `getFontMetrics(fm)` returns the recommended line
  spacing, which is `descent - ascent + leading`. `getFontSpacing()` is
  `getFontMetrics(null)`. `ascent()` is "the distance above (negative) the
  baseline". `descent()` is "the distance below (positive) the baseline".
  For `FontMetricsInt`, Android uses floor for ascent/top and ceil for
  descent/bottom. The current code (225-231) already does this.
- `measureText(text, start, end)`: the advance width of the run, as a float
  (not ink bounds; trailing spaces count).
  `measureText(char[] text, int index, int count)`.
- `getTextWidths(…, float[] widths)`: the advance of each UTF-16 code unit.
  The String overload returns "the number of code units in the specified
  text". A surrogate pair gets its advance on the first unit and 0 on the
  second. A ligature or cluster gets its advance on the first char and 0 on
  the rest.
- `breakText(char[] text, int index, int count, float maxWidth, float[] measuredWidth)`:
  "Measure the text, stopping early if the measured width exceeds maxWidth."
  It returns "the number of chars that were measured. Will always be
  <= abs(count)". "If count is negative, then the characters are measured in
  reverse order." `measuredWidth` is optional: if not null, element [0] gets
  the width measured. `breakText(CharSequence, start, end, boolean
  measureForwards, …)`: "If true, measure forwards, starting at start.
  Otherwise, measure backwards, starting with end."
  `breakText(String, boolean measureForwards, float maxWidth, float[])`
  works on the whole string.
- `getTextBounds(text, start, end, Rect)`: "the smallest rectangle that
  encloses all of the characters, with an implied origin at (0,0)". These are
  **ink** bounds relative to the baseline start, so `top` is negative. The
  text alignment is ignored.
- `getRunAdvance(char[]/CharSequence text, int start, int end, int contextStart, int contextEnd, boolean isRtl, int offset)`:
  "width measurement between start and offset".
  `getOffsetForAdvance(…, boolean isRtl, float advance)` returns the
  "index of offset" whose advance from start is closest to `advance`.
- `drawText(text, x, y, paint)`: `y` is the **baseline**. `x` is the left,
  centre or right of the advance box, depending on `Paint.Align`.
- `setTypeface(tf)` returns `tf` ("As a convenience, the parameter passed is
  also returned"). `getTypeface()` returns null until one is set. `reset()`
  sets it back to null.
- Typeface: the style constants are NORMAL=0, BOLD=1, ITALIC=2, BOLD_ITALIC=3.
  `getStyle()` returns the style bits. Weights run 1..1000, with 400 normal
  and 700 bold. Common Android family names: `sans-serif`,
  `sans-serif-thin` (100), `-light` (300), `-medium` (500), `-black` (900),
  `sans-serif-condensed`, `serif`, `monospace`, `serif-monospace`. On failure
  `createFromAsset` throws `RuntimeException("Font asset not found " + path)`
  and `createFromFile` throws `RuntimeException("Font not found " + path)`.
- TextView `setTypeface(tf, style)`: if `style > 0` it uses
  `tf = (tf == null) ? Typeface.defaultFromStyle(style) : Typeface.create(tf, style)`
  and `setTypeface(tf)`. Then it sets `need = style & ~tf.getStyle()`,
  `paint.setFakeBoldText((need & BOLD) != 0)` and
  `paint.setTextSkewX((need & ITALIC) != 0 ? -0.25f : 0)`.
  Otherwise it calls `setFakeBoldText(false)`, `setTextSkewX(0)` and `setTypeface(tf)`.
- TextView sizes: `setTextSize(float)` means **SP**.
  `setTextSize(unit, size)` converts with
  `TypedValue.applyDimension(unit, size, displayMetrics)`. `getTextSize()`
  returns **pixels**. XML `textSize` is already in pixels.
- TextView lines: XML `android:ellipsize` uses the values none=0, start=1,
  middle=2, end=3, marquee=4. These happen to equal `PangoEllipsizeMode`
  NONE=0, START=1, MIDDLE=2, END=3. Note that Java `TextUtils.TruncateAt`
  ordinals are START=0, MIDDLE=1, END=2, MARQUEE=3, END_SMALL=4, so do
  **not** pass the ordinal through. The AOSP constructor does
  `if (singleLine && getKeyListener() == null && ellipsize == ELLIPSIZE_NOT_SET) ellipsize = ELLIPSIZE_END;`.
  `maxLines=N` without ellipsize clips at N lines with no "…".
  `lines=N` means exactly N lines tall.
- androidx fonts: `TypefaceCompat` picks its implementation from
  `Build.VERSION.SDK_INT`: API 31+ uses Api31Impl, 29-30 Api29Impl, 28
  Api28Impl, 26-27 Api26Impl, 24-25 Api24Impl (if usable), otherwise
  Api21Impl. ATL's SDK_INT depends on the APK
  (`src/api-impl/android/os/Build.java:130-148`). **Api21Impl** copies each
  font resource to a temp file, calls
  `FontFamily.addFontWeightStyle(String path, int weight, boolean italic)` by
  reflection, **deletes the temp file right after**, and then calls
  `Typeface.createFromFamiliesWithDefault(FontFamily[])`. **Api26Impl**
  uses `FontFamily()`,
  `addFontFromAssetManager(AssetManager, String path, int cookie, boolean isAsset, int ttcIndex, int weight, int italic, FontVariationAxis[])`
  (resource fonts are passed with `isAsset=false`, which means use
  `openNonAsset`), `addFontFromBuffer(ByteBuffer, int, FontVariationAxis[], int weight, int italic)`,
  `freeze()`, `abortCreation()` and
  `createFromFamiliesWithDefault(FontFamily[], int weight, int italic)`.
  Api28 also needs `createFromFamiliesWithDefault(FontFamily[], String fallbackName, int, int)`.
  Weight or italic -1 means "read it from the font file".

## Pango/GTK facts you need (no web)
- `pango_layout_get_extents(layout, &ink, &logical)` returns Pango units
  (divide by `PANGO_SCALE`, 1024). `logical.width` is the advance.
  `pango_layout_get_baseline(layout)` is the distance from the layout top to
  the first baseline. Convert a layout-space y to baseline-relative with
  `y - baseline`. Use `pango_layout_get_extents` (float precision), not the
  `_pixel_` variant, for `measureText`.
- Per-character advances: iterate with `PangoLayoutIter *it = pango_layout_get_iter(layout)`.
  For each char, `pango_layout_iter_get_index(it)` gives its UTF-8 byte
  index and `pango_layout_iter_get_char_extents(it, &r)` gives its logical
  rect ("Only logical extents can sensibly be obtained for characters").
  Advance with `pango_layout_iter_next_char(it)` until it returns FALSE.
  Stop when the index reaches the text length. Free the iterator with
  `pango_layout_iter_free`.
- `pango_layout_set_single_paragraph_mode(layout, TRUE)` stops `\n` from
  starting new lines when measuring single-run text.
- To convert a Java string to real UTF-8:
  `const jchar *u = (*env)->GetStringChars(env, s, NULL); jsize n = (*env)->GetStringLength(env, s);
  char *utf8 = g_utf16_to_utf8((const gunichar2 *)u, n, NULL, NULL, NULL); (*env)->ReleaseStringChars(env, s, u);`
  then `g_free(utf8)`. A char whose `g_utf8_get_char` value is > 0xFFFF
  takes 2 UTF-16 units.
- `pango_font_map_add_font_file(PangoFontMap *map, const char *filename, GError **err)`
  exists **since Pango 1.56** ("added fonts take precedence over preexisting
  fonts with the same name"). Check the version on the build runner with
  `rig sh 'pkg-config --modversion pango'`. With an older Pango, use
  `FcConfigAppFontAddFile(FcConfigGetCurrent(), (const FcChar8 *)path)`
  followed by `pango_fc_font_map_config_changed(PANGO_FC_FONT_MAP(map))`
  (`#include <pango/pangofc-fontmap.h>`, meson `dependency('pangoft2')`).
  Get the map with `pango_context_get_font_map(gtk_widget_get_pango_context(window))`.
- A font file's own family, weight and slant come from fontconfig:
  `int n; FcPattern *p = FcFreeTypeQuery((const FcChar8 *)path, 0, NULL, &n);`, then
  `FcPatternGetString(p, FC_FAMILY, 0, &fam)`,
  `FcPatternGetInteger(p, FC_WEIGHT, 0, &w)` (convert with `FcWeightToOpenType(w)`)
  and `FcPatternGetInteger(p, FC_SLANT, 0, &s)` (italic when `s != FC_SLANT_ROMAN`).
  Finally `FcPatternDestroy(p)`. `libtranslationlayer_so` does **not** link
  fontconfig yet: add `dependency('fontconfig')` to its list at
  `meson.build:244-246` (only the main executable has it, at line 271).
- GtkLabel: `gtk_label_set_ellipsize(label, PangoEllipsizeMode)`,
  `gtk_label_set_lines(label, n)` ("the number of lines to which an
  ellipsized, wrapping label should be limited". It has **no effect** unless
  the label wraps and ellipsizes; -1 means no limit), `gtk_label_set_wrap`,
  `gtk_label_set_single_line_mode`. Label attributes are merged with
  `pango_attr_list_change` (copy the existing list first, as C 96-104 does).
  `pango_attr_size_new_absolute(px * PANGO_SCALE)` sets pixels.
  `pango_attr_family_new(const char *)`, `pango_attr_weight_new(PangoWeight)`
  (OpenType numbers 100..900) and `pango_attr_style_new(PANGO_STYLE_ITALIC)`
  set the font. `pango_attr_line_height_new(double factor)` and
  `pango_attr_line_height_new_absolute(int)` are available since Pango 1.50.

## Implementation steps (do them in order; one commit and one gate per step)
Each new Java `native` method needs its JNI declaration in the committed
header under `src/api-impl-jni/generated_headers/` (AGENTS.md "ATL fork
gotchas"). Copy this format exactly, insert it before the closing
`#ifdef __cplusplus`, and remove the block for any native you delete or
rename. Instance natives take `jobject`, static natives take `jclass`. In
the C name, `_` becomes `_1`.
```
/*
 * Class:     android_graphics_Paint
 * Method:    native_measure_text
 * Signature: (JLjava/lang/String;)F
 */
JNIEXPORT jfloat JNICALL Java_android_graphics_Paint_native_1measure_1text
  (JNIEnv *, jclass, jlong, jstring);
```
No lambdas in Java (dx). Don't overload natives; give each one a unique name.

1. **TextView size in pixels** (safe; affects every app).
   In Java, rename `public native void setTextSize(float)` (TextView.java:161) to
   `private native void native_setTextSizePx(float px)`. In the header
   (android_widget_TextView.h:250-254), replace the `setTextSize` block with
   `Java_android_widget_TextView_native_1setTextSizePx (JNIEnv *, jobject, jfloat)`, signature `(F)V`.
   In C (91-107), use `pango_attr_size_new_absolute(px * PANGO_SCALE)` and
   return early unless `GTK_IS_BOX(box)` and the label is a `GtkLabel`
   (copy C 45-53). Add a private `setRawTextSize(float px)` that calls
   `paint.setTextSize(px)` and the native. `setTextSize(float sp)` calls
   `setTextSize(TypedValue.COMPLEX_UNIT_SP, sp)`. `setTextSize(unit, size)`
   calls `setRawTextSize(TypedValue.applyDimension(unit, size, getContext().getResources().getDisplayMetrics()))`.
   Lines 78 and 98 call `setRawTextSize(...)`. `getTextSize()` (329) returns
   `paint.getTextSize()`. Leave the subclass `{}` overrides alone.
2. **Paint quick fixes, Java only** (safe). `align` defaults to `Align.LEFT`
   (24, 415). `ascent()` and `descent()` call
   `native_get_font_metrics(paint, fm)` with a new FontMetrics and return
   `fm.ascent` or `fm.descent`. Add `public float getFontSpacing() { return native_get_font_metrics(paint, null); }`.
   Fix `Canvas.drawText(char[] text, int index, int count, …)` to
   `drawText(new String(text, index, count), x, y, paint)`.
3. **measureText** (safe, one native). Add a static helper in
   android_graphics_Paint.c:
   `static PangoLayout *paint_layout_new(JNIEnv *env, struct AndroidPaint *paint, jstring text)`.
   It does the real UTF-8 conversion (above), `pango_layout_new(ctx)`,
   `set_font_description(paint->font)` and `set_single_paragraph_mode(TRUE)`.
   If `window` is NULL (measuring before the first window), `ctx` is a
   static context from `pango_font_map_create_context(pango_cairo_font_map_get_default())`.
   Add `private static native float native_measure_text(long paint, String text)`:
   build the layout, call `pango_layout_get_extents(l, NULL, &logical)` and
   return `logical.width / (float)PANGO_SCALE`. All Java `measureText`
   overloads call it on the substring. Empty text returns 0 without calling native.
4. **getTextWidths + breakText + getRunAdvance** (safe; one native, the
   rest in Java). Add
   `private static native void native_get_text_widths(long paint, String text, float[] widths)`
   (signature `(JLjava/lang/String;[F)V`, C parameter `jfloatArray`).
   Iterate chars as above and write to a `g_new0(jfloat, len16)` buffer:
   `width = fabsf(r.width) / PANGO_SCALE`, plus an extra 0 after a
   non-BMP char. Copy it out with `SetFloatArrayRegion`. Java: all four
   `getTextWidths` overloads (`String`, `String,int,int`,
   `char[],int,int`, `CharSequence,int,int`) fill `widths` starting at
   index 0 and return the code-unit count. Implement `breakText` on top of
   the widths: forwards, add widths while `sum + w <= maxWidth`; backwards
   (`count < 0` or `!measureForwards`), the same from the end. Set
   `measuredWidth[0]` when it is not null.
   `getRunAdvance(text, start, end, ctxStart, ctxEnd, isRtl, offset)` = sum
   of widths over [start, offset). `getOffsetForAdvance` returns the first
   offset where the running sum reaches the advance (pick the nearer edge).
   Provide both the `char[]` and `CharSequence` overloads. LTR only: add a
   `// TODO RTL` and don't pretend otherwise.
5. **getTextBounds ink rect** (safe). In C 171-191, use the helper and
   `pango_layout_get_pixel_extents(layout, &ink, NULL)`. Subtract the
   baseline from `ink.y` and remove the alignment shift (182-185).
   Check: for "x", `top` is about `-xHeight` and `bottom` is about 0.
6. **TextView maxLines / ellipsize / singleLine** (medium; needs care with
   layout). Java fields: `maxLines = Integer.MAX_VALUE`,
   `TextUtils.TruncateAt ellipsize = null`, `boolean singleLine`.
   Add `private native void native_setLineLimit(int maxLines, int pangoEllipsize)`
   (`(II)V`). Use the same box/label guard as in step 1. In C: if
   `ellipsize != 0 && maxLines == 1`, call `wrap(FALSE)`,
   `single_line_mode(TRUE)` and `set_ellipsize(mode)`. Else if
   `ellipsize != 0`, call `wrap(TRUE)`, `set_ellipsize(mode)` and
   `set_lines(maxLines)` (-1 if unlimited). Else, `wrap(TRUE)`,
   `ellipsize NONE`, `lines -1`, `single_line_mode(FALSE)`. Java
   setters (`setMaxLines`, `setLines` (treat as max for now),
   `setSingleLine()`/`setSingleLine(boolean)` meaning maxLines 1,
   `setEllipsize`) update the fields and call one `applyLineLimit()`. That
   method maps TruncateAt START→1, MIDDLE→2, END/END_SMALL/MARQUEE→3,
   null→0. The getters (432, 538) return the fields. In the constructor,
   read `TextView_maxLines`, `TextView_lines`, `TextView_singleLine` and
   `TextView_ellipsize` (an int; map 1..4 to TruncateAt). Apply AOSP's
   singleLine⇒END default only when the class is not an EditText. Do **not**
   fake `maxLines` without ellipsize by adding "…". Leave that case
   unlimited and note it (see step 11).
7. **Typeface model + Paint.setTypeface** (medium). Typeface gets
   `String family` (null means the system default; also treat
   `"sans-serif"` as null so Paint and GtkLabel use the same default font),
   `int weight` and `boolean italic`. `getStyle()` returns
   `(weight >= 600 ? BOLD : 0) | (italic ? ITALIC : 0)`. Add `isBold()`,
   `isItalic()` and `getWeight()`. Constants: DEFAULT (400),
   DEFAULT_BOLD (700), SANS_SERIF, SERIF ("serif"), MONOSPACE ("monospace").
   `create(String name, int style)` maps the Android names listed above to
   family plus weight. `create(Typeface, int style)`,
   `create(Typeface, int weight, boolean italic)` and `defaultFromStyle`
   return new objects; never mutate the shared constants. Keep the public
   `native_instance` field (androidx reads it). In Paint, add a field
   `typeface`. `setTypeface` stores it, calls
   `native_set_typeface(long paint, String family, int weight, boolean italic)`
   (`(JLjava/lang/String;IZ)V`) and returns it. In C,
   `pango_font_description_set_family` /
   `unset_fields(PANGO_FONT_MASK_FAMILY)` when family is NULL,
   `set_weight((PangoWeight)weight)` and
   `set_style(italic ? PANGO_STYLE_ITALIC : PANGO_STYLE_NORMAL)`. Null tf
   means family unset, 400, normal. `getTypeface` returns the field.
   Copy it in `Paint(Paint)`/`set()`, and clear it in `reset()`.
8. **TextView typeface** (medium). Add
   `private native void native_setFont(String family, int weight, boolean italic)`
   (`(Ljava/lang/String;IZ)V`, with the box/label guard), which sets family,
   weight and style attributes via `pango_attr_list_change`.
   `setTypeface(tf)` stores the field, calls `paint.setTypeface(tf)` and
   calls the native. `setTypeface(tf, style)` follows the AOSP rule above.
   Use a real weight bump in place of fake bold. Delete the CSS-class
   switch (201-218). `getTypeface()` returns the field. In the constructor,
   read `TextView_typeface` (int: 1 sans, 2 serif, 3 monospace),
   `TextView_fontFamily` (if it is a string, `Typeface.create(name, style)`;
   a `@font/` reference waits for step 10) and `textStyle`, plus the same
   three attributes from TextAppearance (75-80).
9. **Font files** (needs care: meson, Pango version, temp files). New
   `src/api-impl-jni/graphics/android_graphics_Typeface.c` (add it to the
   source list near `meson.build:183`) and a new header
   `generated_headers/android_graphics_Typeface.h` (copy the guard
   boilerplate from android_graphics_Paint.h). Native:
   `private static native String native_register_font(String path, int[] weightItalic)`
   (`(Ljava/lang/String;[I)Ljava/lang/String;`). It calls
   `FcFreeTypeQuery` to get the family, weight and italic flag, registers the
   file (`pango_font_map_add_font_file` or the fontconfig fallback), writes
   `{weight, italic}` and returns the family, or NULL on failure.
   Java: `createFromFile(path)` registers the file in place.
   `createFromAsset(mgr, path)` copies `mgr.open(path)` to
   `/tmp/atl_cache/fonts/<hash of path>.ttf` (the same root as
   `Context.getCacheDir`, Context.java:290-292) and registers that copy;
   throw the Android RuntimeExceptions on failure. `Typeface.Builder(mgr, path)`
   behaves the same. `FontFamily`: each `add*` method copies its input
   **immediately** into the cache dir (androidx deletes its temp file right
   after the call), registers it, and stores the result.
   `addFontFromAssetManager` uses `isAsset ? mgr.open(path) : mgr.openNonAsset(cookie, path)`.
   `addFontFromBuffer` writes out the buffer's bytes. The methods return
   false on failure. `createFromFamiliesWithDefault(families[, w, i])` and
   the new overload `(FontFamily[], String, int, int)` return a Typeface
   for the first family that registered any face, using the requested
   weight/italic, or the face's own values when they are -1. Register a
   given file only once (keep a `HashMap<String,String>` from path to family).
10. **Resources fonts** (Claude). Add `Resources.getFont(int)` and
   `TypedArray.getFont(int)` (both missing). A `res/font` entry is either a
   .ttf/.otf file (step 9 copy and register) or a `<font-family>` XML with
   `<font android:font= android:fontWeight= android:fontStyle=>` children.
   The XML can also be a downloadable-font provider query, which must fall
   back to default and never throw. Then handle `TextView_fontFamily` given
   as a `@font/` reference. androidx on SDK 29+ needs
   `android.graphics.fonts.Font`/`FontFamily` builders and
   `Typeface.CustomFallbackBuilder`. These are new classes: add them to
   `src/api-impl/meson.build`.
11. **maxLines without ellipsize, `lines=N`, lineSpacing** (Claude). Clip
   at N lines with no "…". GtkLabel can't do this itself, so it needs a
   measure clamp (N × line height) plus `gtk_widget_set_overflow(HIDDEN)`
   on the box. `setLineSpacing(add, mult)` and the XML
   `lineSpacingExtra`/`Multiplier` attributes map to
   `pango_attr_line_height_new(mult)` when add == 0, and otherwise to
   `_absolute((int)(((descent - ascent) * mult + add) * PANGO_SCALE))`.

## Failing checks to write first (docs/agent-tasks/test-first.md)
Each check must FAIL on the unfixed fork, twice in the same way. Get
coordinates from `rig smoke work/apks/<apk> --settle 20` screenshots
(1080x1920).
- **Paint numbers (steps 2-5), via jdb**, deterministic, no OCR:
  `rig -r mini sh 'scripts/runner/jdb-app.sh org.fossify.math "android.widget.TextView.setText" "140,1055" "print new android.graphics.Paint().measureText(\"WWWW\");print new android.graphics.Paint().measureText(\"iiii\");print new android.graphics.Paint().descent();print new android.graphics.Paint().breakText(\"hello world\", true, 30f, null)"'`
  then read `/tmp/jdb-out.txt`. Today: W and i are equal (`4*size*0.6`),
  descent is 0 and breakText is 10. Fixed: W > i, descent > 0 and
  breakText < 11. Record the before and after values in the commit message.
- **Text size (step 1)**: `interactions/org.fossify.math.txt` already
  checks 7+8=15. Re-run it after step 1: the digits must still be read
  (`expect-text "15"`). Compare `shot` crops of a button label before and
  after: the glyph height should shrink by about 25 %.
- **Ellipsize (step 6)**: pick a corpus app whose row layout uses
  `maxLines="1"`/`ellipsize="end"`. Check with
  `rig sh 'unzip -p work/apks/<pkg>_*.apk res/layout/<item>.xml | strings | head'`
  or the decompiled layout. Fossify File Manager and Contacts list rows are
  likely candidates. Create a long name in the app via the script (`type`),
  then `shot` and `expect-text` on the text of the **next** row. That check
  fails today because the wrapped row covers it.
- **Custom font (steps 7-9)**: an app found with the `unzip -l` loop above
  that sets a font from assets or res/font on a visible label. Add
  `expect-text` on a label that only reads correctly in the custom font, or
  compare `shot`s before and after by eye. If no corpus app shows a font
  visibly, say so in the commit and rely on the jdb check
  `print android.graphics.Typeface.DEFAULT_BOLD.getStyle()`, which must be 1.

## Done when
Per step: `rig build --worktree ~/Develop/android-translation-layer` gives
BUILD OK. Then `rig atlh --detach <id> run --run-id <id>` followed by
`rig job wait <id>`, then `rig atlh compare <base> <id>` shows no
regressions, and the step's check above passes. The jdb numbers match
Android's semantics (W>i, descent>0, sensible breakText), one-line rows show
"…", and `DEFAULT_BOLD` draws bold. Mark the triage rows (120, 123, 161,
169, 172) as done in docs/agent-tasks/triage.md and update HANDOFF.md.

## Risks / what not to do
- Don't replace guesses with different guesses (`size*0.55`, constants). If
  Pango can't answer, leave a TODO and say so.
- Measuring and drawing must use the same PangoContext and font description.
  Otherwise centred text is off by sub-pixels and `breakText` disagrees with
  `drawText`.
- Thread safety: `gtk_widget_get_pango_context(window)` is a main-thread
  object. Apps measure on background threads (RecyclerView prefetch,
  Glide). The existing code already has this risk. Don't make it worse (no
  global mutable layout cache). If a crash points here, use a per-thread
  context.
- Don't pass `TruncateAt.ordinal()` as a Pango mode (off by one).
- Don't break Button/EditText/CheckBox: every new TextView native needs the
  box/label guard. Their own widgets are a separate task.
- Don't register the same font file repeatedly (it grows fontconfig). Don't
  register app fonts under the name `sans-serif`, which would hijack the
  UI font for the whole process.
- Keep the step 1 unit change in one commit: half-converted sizes (XML in
  px, code in sp) are worse than either.
- Nothing goes upstream (AGENTS.md hard rules).
