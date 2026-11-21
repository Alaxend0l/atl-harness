<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: Bitmap pixels, colour filters, Xfermodes, saveLayer and BitmapFactory sampling

Fork: `~/Develop/android-translation-layer` (paths below are relative to it).
Read `doc/Shaders.md` first: shapes with a shader are drawn as an alpha mask
(`gtk_snapshot_push_mask`) over the shader, and Android matrices must go
through `snapshot_transform_android_matrix()` (AndroidPaint.h), never
`gtk_snapshot_transform_matrix` (the Cairo renderer paints that hot pink).
Line numbers are from fork `main` at 9a2cd6c4.

## Symptom / why it matters
- **Crash**: `Bitmap.getPixel()` and any partial `getPixels()` call `exit(1)`
  (whole app dies, no Java exception). Colour pickers, eyedroppers, palette
  code, games that hit-test against bitmaps.
- **Wrong colours**: `setPixels()` swaps red and blue; `getPixels()` returns
  premultiplied values; `createBitmap(int[] colors, ...)` ignores the colours
  (blank QR codes and generated images).
- **Untinted icons**: `BlendModeColorFilter` (what androidx uses for tints on
  API 29+), `LightingColorFilter` and `ColorMatrixColorFilter` (greyed-out /
  disabled images) are dropped; every `PorterDuffColorFilter` mode is drawn as
  SRC_IN. `drawBitmap` ignores `Paint.setAlpha` (fades, disabled states).
- **Cut-outs and erasers**: `Paint.setXfermode` is ignored on all draws except
  that `GskCanvas.drawRect` drops the whole rect, and on a Bitmap canvas a CLEAR
  rect/round rect erases the *entire* bitmap. Glide's `circleCrop()` (circle
  then `drawBitmap` with SRC_IN) gives square images; an eraser (Fossify Paint)
  with CLEAR/DST_OUT does nothing or wipes everything.
- `drawColor(color, PorterDuff.Mode)` and `drawARGB` are log stubs, so the
  common `drawColor(0, PorterDuff.Mode.CLEAR)` "clear the canvas" never clears.
- `saveLayer` ignores its Paint (alpha, xfermode, colour filter) and
  `saveLayerAlpha` does not exist (NoSuchMethodError). Lottie mattes, fading
  edges, "draw content, then DST_IN a mask" rounded avatars break.
- `BitmapFactory`: `inSampleSize` is ignored (camera photos decoded at full
  size: slow, memory-heavy; Glide's Downsampler relies on it) and
  `inJustDecodeBounds` decodes the whole image; a decode failure passes NULL
  to `gdk_texture_new_for_pixbuf` (crash).

## Current code
**Bitmap** (`src/api-impl/android/graphics/Bitmap.java`,
`src/api-impl-jni/graphics/android_graphics_Bitmap.c`)
- Bitmap.java 230-238: `getPixels`/`getPixel` call `native_get_pixels`;
  Bitmap.c 119-129 `exit(1)`s unless the read is the whole texture, then
  `gdk_texture_download` (default format = `GDK_MEMORY_B8G8R8A8_PREMULTIPLIED`:
  channel order right, but premultiplied).
- Bitmap.c 156-168 `native_set_pixels`: builds a `GDK_MEMORY_R8G8B8A8` texture
  from Java ints (memory order of a little-endian ARGB int is B,G,R,A, so red
  and blue swap); `memcpy`s `height*stride` ints (reads past the array when
  `stride > width` on the last row); appends the texture *over* the old
  content (SRC_OVER, so a translucent pixel blends instead of replacing).
- Bitmap.java 98-100: `createBitmap(int[] colors, w, h, config)` ignores
  `colors`; the `(int[] colors, int offset, int stride, w, h, config)` overload
  is missing.
- Bitmap.java 133-140: `getSnapshot()` bumps `generationId`; 125-131
  `getTexture()` renders the snapshot (Bitmap.c 24-80: main thread = GPU
  renderer on an offscreen display, other threads = `gsk_cairo_renderer_new`).

**Colour filters** (`Paint.java`, `android_graphics_Paint.c`, `android_atl_GskCanvas.c`)
- Paint.java 125-134: only `PorterDuffColorFilter` reaches native (`mode.ordinal()`),
  anything else calls `native_set_color_filter(paint, -1, 0)` = no filter.
- Paint.c 154-167: always builds the SRC_IN matrix (rgb := filter rgb,
  a := a * filter alpha) whatever the mode.
- `LightingColorFilter.java` and `ColorMatrixColorFilter.java` store nothing;
  `BlendModeColorFilter.java` 26-82 is never looked at by Paint.
- GskCanvas.c 10-40 (both `native_drawBitmap`): colour matrix yes, paint alpha no.

**Xfermode / drawColor** (`src/api-impl/android/atl/GskCanvas.java`, `Canvas.java`)
- GskCanvas.java 123-129: `drawRect` returns early for *any* xfermode (TODO
  mentions `gtk_snapshot_push_composite`). No other draw looks at the xfermode.
- Canvas.java 83-97 and 457-473: on a Bitmap canvas, CLEAR in drawRect /
  drawRoundRect does `restoreToCount(1)` + `bitmap.eraseColor(0)` + re-`save()`:
  erases the whole bitmap and loses the matrix and clips of the open saves.
- Canvas.java 545-553 `drawColor(int)` ok, `drawARGB` stub; 571-573
  `drawColor(int, PorterDuff.Mode)` stub. 634-638 `drawPaint` = drawRect of
  ±100000.

**saveLayer** (`Canvas.java` 555-565): three overloads, all `return save();`.
No 6-arg `saveLayer(l,t,r,b,paint,flags)`, no `saveLayerAlpha`. GskCanvas.java
27-45: `save()` = `gtk_snapshot_save`; `restore()` pops the clips counted in
`push_history[saveCount]` (clipRect/clipPath 83-107) then `gtk_snapshot_restore`.

**BitmapFactory** (`BitmapFactory.java`, `android_graphics_BitmapFactory.c`)
- BitmapFactory.c 57-66: `gdk_pixbuf_new_from_stream` (full decode, no NULL
  check), `opts` unused.
- BitmapFactory.java 588-613: sets `outWidth/outHeight` from the full decode,
  then for `inJustDecodeBounds` recycles it and returns null. `inSampleSize`
  never read; `outMimeType` never set.

## What Android does
- `Bitmap.getPixel(x,y)`: "non-premultiplied ARGB value in the sRGB color
  space"; throws IllegalArgumentException if x/y out of range.
  `getPixels(pixels, offset, stride, x, y, w, h)`: same format, row i goes to
  `pixels[offset + i*stride ...]`; IllegalArgumentException if the rect leaves
  the bitmap, ArrayIndexOutOfBoundsException if the array is too small.
  `setPixel(s)`: takes non-premultiplied colours and **replaces** the pixels
  (alpha too); IllegalStateException if immutable.
- `Paint` xfermode (Skia): inside the shape's coverage the result is
  `mode(src, dst)`, outside it dst is untouched. `dst` is the current layer
  (the bitmap, or the innermost saveLayer). PorterDuff (premultiplied):
  CLEAR [0,0]; SRC [Sa,Sc]; DST_OUT [Da*(1-Sa), Dc*(1-Sa)]; SRC_IN [Sa*Da,
  Sc*Da]; DST_IN [Da*Sa, Dc*Sa]; etc.
- `drawColor(color, mode)` = fill the whole clip with `color` using `mode`;
  `drawARGB(a,r,g,b)` = `drawColor(Color.argb(a,r,g,b))` (SRC_OVER).
- `saveLayer(bounds, paint)`: same as save(), plus everything until the
  matching restore goes to an offscreen layer (clipped to bounds, starting
  transparent); on restore the layer is composited with the paint's alpha,
  colour filter and xfermode. null paint = plain SRC_OVER.
  `saveLayerAlpha(bounds, alpha)` = saveLayer with a paint of that alpha.
  Returns the save count before the call, like save().
- Colour filters work on **unpremultiplied** colours. ColorMatrix is a 4x5
  row-major array, `R' = a[0]*R + a[1]*G + a[2]*B + a[3]*A + a[4]` with offsets in
  0..255. `LightingColorFilter(mul, add)`: `R' = R*mul.r/255 + add.r` (same for
  G, B), alpha unchanged. PorterDuffColorFilter / BlendModeColorFilter: filter
  colour = src, pixel = dst. Unpremultiplied per pixel (s = filter rgb, sa =
  filter alpha, d/da = pixel):
  SRC_IN: rgb=s, a=sa*da | SRC_ATOP: rgb=(1-sa)*d+sa*s, a=da |
  MULTIPLY: rgb=s*d, a=sa*da | DST_IN: rgb=d, a=da*sa | DST_OUT: rgb=d,
  a=da*(1-sa) | SRC_OUT: rgb=s, a=sa*(1-da) | SRC: rgb=s, a=sa | DST:
  identity | CLEAR: all 0. All of these are linear, so one colour matrix each.
  SCREEN, OVERLAY, DARKEN, LIGHTEN, ADD, XOR, SRC_OVER, DST_OVER are not.
- `BitmapFactory.Options.inSampleSize`: "Any value <= 1 is treated the same as
  1 ... rounded down to the nearest power of 2". AOSP `BitmapFactory.cpp`
  `doDecode`: `if (sampleSize <= 0) sampleSize = 1;`, size =
  `codec->getSampledDimensions(sampleSize)`; it then sets `outWidth`,
  `outHeight`, `outMimeType` ("image/png", "image/jpeg", "image/webp",
  "image/gif", "image/bmp", ...) and returns null `if (onlyDecodeSize)`;
  density scaling (`scale = targetDensity / density`) is applied only *after*
  that return. So outWidth/outHeight = sampled size, no density scaling, in
  both modes. On error outWidth = outHeight = -1, outMimeType = null.

## GSK building blocks (GTK 4.22, runners have it; guard with `GTK_CHECK_VERSION(4, 22, 0)`)
- `gtk_snapshot_push_composite(snapshot, GskPorterDuff op)`: "Until the first
  call to gtk_snapshot_pop(), the mask image for the mask operation will be
  recorded. After that call, the child image will be recorded until the second
  call to gtk_snapshot_pop()." GskCompositeNode: "uses Porter/Duff compositing
  operators to combine its child with the background ... wherever the mask is
  set". That is Skia's coverage rule: mask = shape, child = the paint's draw.
  `GskPorterDuff`: SOURCE 0, DEST 1, SOURCE_OVER_DEST 2, DEST_OVER_SOURCE 3,
  SOURCE_IN_DEST 4, DEST_IN_SOURCE 5, SOURCE_OUT_DEST 6, DEST_OUT_SOURCE 7,
  SOURCE_ATOP_DEST 8, DEST_ATOP_SOURCE 9, XOR 10, CLEAR 11.
- `gtk_snapshot_push_isolation(snapshot, GSK_ISOLATION_BACKGROUND)` (one pop):
  `GSK_ISOLATION_BACKGROUND`: "If the background is not available, future
  operations will be rendered to a transparent background and added to the
  existing background later." = a saveLayer.
- Cairo (Bitmaps drawn off the main thread): `gsk_composite_node_draw` is
  real Cairo code (push_group + `CAIRO_OPERATOR_*`, background =
  `cairo_get_group_target`), `gsk_isolation_node_draw` does `cairo_push_group`
  for BACKGROUND. Opacity, colour matrix, mask, clip also draw under Cairo.
  Do **not** use `gtk_snapshot_push_copy`/`append_paste` (unstable).
- `gsk_color_matrix_node`: "pixel = transpose(color_matrix) * pixel +
  color_offset ... operates on unpremultiplied colors ... R, G, B, A", offsets
  in 0..1. With `graphene_matrix_init_from_float(&m, f)` (row-major `f`), output
  channel i = sum over j of `f[j*4 + i] * in_j`. So from an Android array `a`:
  `f[j*4 + i] = a[i*5 + j]` (i, j in 0..3), `offset[i] = a[i*5 + 4] / 255`.

## Implementation steps (in order of user impact; one commit per step)
Every new Java `native` method needs its prototype added by hand to the
committed header in `src/api-impl-jni/generated_headers/` (copy an existing
block: comment with Class/Method/Signature, then `JNIEXPORT ...`; static methods
take `jclass`, instance methods `jobject`; `_` in a name becomes `_1`).
No lambdas in `src/api-impl` (dx). No new header for steps that keep signatures.

1. **Bitmap.c `native_get_pixels`** (119-129): remove the partial-read
   `exit(1)`; require x=0,y=0,full size (Java guarantees it, step 2). Download
   with `GdkTextureDownloader *d = gdk_texture_downloader_new(texture);
   gdk_texture_downloader_set_format(d, GDK_MEMORY_B8G8R8A8);` (straight alpha,
   = Android ARGB ints on little-endian), `gdk_texture_downloader_download_into(d,
   (guchar *)(array + offset), stride * 4); gdk_texture_downloader_free(d);`.
   Same signature, no header change.
2. **Bitmap.java `getPixels`/`getPixel`** (230-238): add `private int[]
   pixelCache; private int pixelCacheGen;`. A helper `int[] allPixels()` returns
   the cache if `pixelCacheGen == generationId`, else allocates `width*height`,
   calls `native_get_pixels(getTexture(), buf, 0, width, 0, 0, width, height)`,
   stores it. `getPixels` checks bounds (Android exceptions above) and copies
   rows with `System.arraycopy`; `getPixel` returns `allPixels()[y*width+x]`.
   Return copies, never the cache itself.
3. **Bitmap.c `native_set_pixels`** (156-168): use `GDK_MEMORY_B8G8R8A8`, copy
   row by row into a `width*height*4` buffer (`array + offset + row*stride`,
   `width` ints per row), texture stride `width*4`.
4. **Bitmap.java `setPixels`/`setPixel`** (267-274): replace semantics. Get
   `allPixels()` copy, write the rect into it, then `native_recycle(texture,
   snapshot); texture = 0; snapshot = native_create_snapshot(0);` (an empty
   snapshot) and `native_set_pixels(snapshot, all, 0, width, 0, 0, width,
   height)`; call `notifyPixelsChanged()`, then store `all` as the cache with
   the new generation. Throw IllegalStateException if `!mutable`.
5. **Bitmap.java `createBitmap(int[] colors, ...)`** (98-100): create, then
   `setPixels(colors, 0, width, 0, 0, width, height)`; add the
   `(int[] colors, int offset, int stride, int width, int height, Config)`
   overload; result immutable (`mutable = false` after filling).
6. **ColorFilter → matrix (Java only).** In `ColorFilter.java` add
   `float[] toColorMatrix() { return null; }` (package-private) and a static
   `porterDuffMatrix(int color, int pdNativeInt)` building the 20-float array
   per the table in "What Android does" (null for non-linear modes). Override
   in `PorterDuffColorFilter` (`mMode.nativeInt`), `BlendModeColorFilter`
   (`mMode.getXfermode().porterDuffMode`), `LightingColorFilter` (store
   mul/add; diagonal `mul.c/255`, offsets `add.c`, alpha row 0,0,0,1,0),
   `ColorMatrixColorFilter` (copy of `matrix.getArray()` / the float[]; add
   `getColorMatrix(ColorMatrix)`, `setColorMatrix(ColorMatrix)`).
7. **Paint colour matrix (Java + C + header).** Add `private static native
   void native_set_color_matrix(long paint, float[] matrix);` to Paint.java and
   `Java_android_graphics_Paint_native_1set_1color_1matrix(JNIEnv *env, jclass
   clazz, jlong paint_ptr, jfloatArray matrix)` to Paint.c (signature
   `(J[F)V`, add to `generated_headers/android_graphics_Paint.h`). NULL →
   `use_color_filter = false`; else transpose into `paint->color_matrix` and
   `color_offset` as in "GSK building blocks". `setColorFilter` (125-134) calls
   it with `colorFilter == null ? null : colorFilter.toColorMatrix()`; when a
   non-null filter returns null log once per mode (`Log.w`) and draw unfiltered.
   Leave `native_set_color_filter` unused (or delete it and its header entry).
8. **drawBitmap alpha** (GskCanvas.c 10-40): if `paint->color.alpha < 1`,
   `gtk_snapshot_push_opacity(snapshot, paint->color.alpha)` outside the colour
   matrix (outermost), pop it last.
9. **C: composite helper** in GskCanvas.c: `jboolean
   Java_android_atl_GskCanvas_native_1pushComposite(JNIEnv *env, jobject this,
   jlong snapshot_ptr, jint pd_mode)` (Java: `protected native boolean
   native_pushComposite(long snapshot, int pdMode);`, signature `(JI)Z`, header
   `android_atl_GskCanvas.h`). Map Android nativeInt → GskPorterDuff: CLEAR(0)
   →CLEAR, SRC(1)→SOURCE, DST(2)→DEST, DST_OVER(4)→DEST_OVER_SOURCE,
   SRC_IN(5)→SOURCE_IN_DEST, DST_IN(6)→DEST_IN_SOURCE, SRC_OUT(7)→
   SOURCE_OUT_DEST, DST_OUT(8)→DEST_OUT_SOURCE, SRC_ATOP(9)→SOURCE_ATOP_DEST,
   DST_ATOP(10)→DEST_ATOP_SOURCE, XOR(11)→XOR. Call `gtk_snapshot_push_composite`
   and return TRUE; return FALSE (push nothing) for SRC_OVER(3), the blend
   modes 12-17 and when `!GTK_CHECK_VERSION(4, 22, 0)`.
10. **GskCanvas.java xfermode draws.** Helper `private int xferMode(Paint p)`:
    `PorterDuffXfermode` → its `porterDuffMode`, else 3 (SRC_OVER). The field
    is package-private and GskCanvas is in `android.atl`: add
    `public int getPorterDuffMode()` to `PorterDuffXfermode.java`. Helper `private Paint maskPaint(Paint p)`:
    `new Paint(p)`, `setShader(null)`, `setColorFilter(null)`,
    `setXfermode(null)`, `setColor(0xFFFFFFFF)` (keeps style/stroke/font).
    In drawRect, drawPath, drawRoundRect, drawText, drawLine, drawLines,
    drawBitmap (both): if `mode != 3 && native_pushComposite(snapshot, mode)`
    then draw the shape with `maskPaint(paint)`, `native_pop(snapshot, 1)`,
    draw normally, `native_pop(snapshot, 1)`. If the push failed: modes
    CLEAR/DST/DST_IN/DST_OUT/DST_ATOP → skip the draw (log once), others →
    draw normally. Remove the early return at 125-127. For drawBitmap the mask
    is the dst rect (`native_drawRect` with the mask paint, FILL style).
11. **Canvas.java**: delete the CLEAR hacks (85-93, 461-468; keep the normal
    draw path). `drawColor(int color, PorterDuff.Mode mode)`: Paint with color
    and `new PorterDuffXfermode(mode)`, `drawPaint(paint)`. Fast path only for
    a Bitmap canvas with `mode == CLEAR`, `getSaveCount() == 1`, identity
    matrix and no clip: `bitmap.eraseColor(0)` (keeps the node tree from growing
    on repeated clears). `drawARGB(a,r,g,b)` → `drawColor(Color.argb(a,r,g,b))`;
    add `drawRGB(r,g,b)`.
12. **C: layer push** in GskCanvas.c: `jint
    Java_android_atl_GskCanvas_native_1pushLayer(JNIEnv*, jobject, jlong
    snapshot, jfloat l, jfloat t, jfloat r, jfloat b, jboolean has_bounds,
    jlong paint_ptr, jint pd_mode)`, signature `(JFFFFZJI)I`, Java
    `protected native int native_pushLayer(long snapshot, float l, float t,
    float r, float b, boolean hasBounds, long paint, int pdMode)`;
    paint_ptr 0 = no paint. Push, outermost first,
    and count: `push_clip(bounds)` if has_bounds; if pdMode is not SRC_OVER
    and supported: `push_composite(op)`, `append_color(white, bounds or
    ±100000 rect)`, `gtk_snapshot_pop` (mask done; child pop pending, count
    it); `push_opacity(alpha)` if paint alpha < 1; `push_color_matrix` if
    `paint->use_color_filter`; `push_isolation(GSK_ISOLATION_BACKGROUND)`
    (4.22 only; before 4.22 skip the composite and the isolation, so layer
    alpha and colour filter still work). Return the number of pending pops.
    Share the nativeInt → GskPorterDuff switch with step 9 (a static function).
13. **GskCanvas.java saveLayer**: `int[] layer_pops` grown like `push_history`.
    Override `saveLayer(float l, float t, float r, float b, Paint paint)`:
    `int ret = save();` then `layer_pops[getSaveCount()] =
    native_pushLayer(snapshot, l, t, r, b, true, paint == null ? 0 :
    paint.paint, paint == null ? 3 : xferMode(paint)); return ret;`.
    `restore()`: pop clips (existing 39-42), then `native_pop(snapshot,
    layer_pops[n])` and zero it, then the rest. Order matters: the layer was
    pushed right after `gtk_snapshot_save`, before any clip of that level.
14. **Canvas.java saveLayer family**: the 5-arg `saveLayer(l,t,r,b,paint)` does
    `synchronized (bitmap) { gsk_canvas.snapshot = bitmap.getSnapshot(); return
    gsk_canvas.saveLayer(l,t,r,b,paint); }`; every other overload (RectF,
    RectF+flags, 6-arg with flags, null bounds → `getClipBounds` or a
    ±100000 rect) calls it. Add `saveLayerAlpha(RectF,int)`,
    `saveLayerAlpha(RectF,int,int)`, `saveLayerAlpha(l,t,r,b,int)`,
    `saveLayerAlpha(l,t,r,b,int,int)`: Paint with `setAlpha(alpha)`. Virtual
    dispatch sends GskCanvas (onscreen, `bitmap == null`) to its override.
15. **BitmapFactory.c `nativeDecodeStream`** (57-66), same signature: replace
    `gdk_pixbuf_new_from_stream` by a `GdkPixbufLoader`. Read options with
    `jclass oc = (*env)->GetObjectClass(env, opts)` and `GetFieldID(oc,
    "inSampleSize", "I")`, `"inJustDecodeBounds", "Z"`. Sample s = largest
    power of 2 ≤ max(1, inSampleSize). `g_signal_connect(loader,
    "size-prepared", cb, &st)`; in cb store w,h and, if s > 1,
    `gdk_pixbuf_loader_set_size(loader, MAX(1, w / s), MAX(1, h / s))`. Loop
    `g_input_stream_read(stream, buf, n, NULL, &err)` →
    `gdk_pixbuf_loader_write`; for bounds-only stop as soon as cb ran. Then
    `gdk_pixbuf_loader_close(loader, &err)` (ignore/clear its error in
    bounds-only mode). Set `outWidth`/`outHeight` (`"I"`, sampled size, -1 on
    failure) and `outMimeType` (`"Ljava/lang/String;"`, first entry of
    `gdk_pixbuf_format_get_mime_types(gdk_pixbuf_loader_get_format(loader))`,
    `g_strfreev` it; the format itself is owned by the loader). Return 0 for
    bounds-only or failure (`gdk_pixbuf_loader_get_pixbuf` NULL), else
    `gdk_texture_new_for_pixbuf`. `g_object_unref(loader)` last.
    BitmapFactory.java 595-606 already returns null for texture 0 without
    touching outWidth; keep 600-601 (decoded size = sampled size).

## Failing checks to write first (docs/agent-tasks/test-first.md)
- **Node dumps (no app UI needed)**: run an app that uses Glide `circleCrop()`
  or `RoundedCorners` (Fossify Music / Gallery / Contacts; check with
  `atlh missing-apis` or jadx) with `[atl.env] ATL_DUMP_NODES=/tmp/nodes`.
  Before: the SRC_IN `drawBitmap` shows as a plain texture after the circle
  (`grep -L composite`); after: a `composite` node. Render the dump with
  `gtk4-rendernode-tool render FILE out.png` and check a corner pixel is
  transparent (`magick out.png -format '%[pixel:p{0,0}]' info:`). Dumps from
  `GskCairoRenderer` must not render pink.
- **Logs**: count `STUB: drawColor` lines in the run log of a few corpus apps
  (must drop to 0); a crash signature for `Bitmap.readPixels: partial read`
  must disappear.
- **Interaction script** for Fossify Paint (`interactions/org.fossify.paint.txt`):
  tap several times to draw dots, `shot drawn`, `expect-changed`; pick the
  eraser, tap the same spots, `shot erased`, `expect-changed`. Must FAIL on the
  unfixed fork (eraser no-op). Real strokes need a `drag` step in the harness
  (xdotool mousedown/mousemove/mouseup): add it to `src/` + docs/usage.md
  first if taps don't draw.
- **BitmapFactory**: with node dumps on, Fossify Gallery thumbnails of a large
  JPEG must be texture nodes of the sampled size (Glide asks for
  inSampleSize ≥ 2), not the full photo size.

## Done when
- `rig build --worktree ~/Develop/android-translation-layer` is clean;
  `rig atlh run` (detached, `rig job wait`) + `rig atlh compare` against the
  previous run shows no regressions (ALIVE→CRASH, ok→FAIL), the new
  interaction script passes, and the checks above flip from fail to pass.
- Run once on `mini` (aarch64) too; nothing renders pink in Cairo dumps.
- Fork commit messages say what changed; harness: script committed, roadmap /
  HANDOFF updated, triage rows (Canvas stubs, getPixel, colour filter,
  drawBitmap alpha, saveLayer, xfermode drawRect, BitmapFactory) marked done.

## Risks / what not to do
- No `gtk_snapshot_transform_matrix`; no `push_copy`/`append_paste`.
- Composite = exactly two pops, mask first; a missing pop makes
  `gtk_snapshot_restore` warn and the bitmap's node tree is lost. Every
  push needs a counted pop on restore and on `restoreToCount`.
- Don't "fake" modes: an unsupported blend xfermode or colour filter must log
  and draw the nearest honest thing (SRC_OVER / unfiltered), never `return`
  silently for modes we can do.
- Replacing a Bitmap's snapshot (steps 4, 11) while a `Canvas` on it has open
  saves/clips desyncs the canvas; only take the eraseColor fast path at save
  count 1 with identity matrix and no clips.
- A CLEAR on the onscreen canvas outside a saveLayer punches through to the
  window (Android HW-accel does the same: black). Don't special-case it.
- `getPixel` in a loop must hit the cache, not download the texture each time.
- Keep `GTK_CHECK_VERSION(4, 22, 0)` fallbacks compiling (`gtk4 >= 4.14` in
  meson.build); test on the runners' 4.22.
- Out of scope: inDensity/inTargetDensity scaling (setDensityFromOptions,
  BitmapFactory.java 513-534, is commented out), RGB_565 configs, EXIF rotation.
