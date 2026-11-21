<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Task: real interpolators: AnimationUtils.loadInterpolator, PathInterpolator, Bounce & co.

For an opencode session with shell access. Line numbers are from fork `main`
`9a2cd6c4`; re-check them in `~/Develop/android-translation-layer` before editing.
Paths below are relative to the fork root.

## Symptom / why it matters
Fossify File Manager and Fossify Music Player abort shortly after drawing (harness
status CRASH; roadmap calls this the "PropertyValuesHolder.setAnimatedValue NPE"
family). Log of `fossify-sp2` / `fossify1` on bazzite (`org.fossify.filemanager.log`):
```
java.lang.NullPointerException: Attempt to invoke interface method 'float android.animation.TimeInterpolator.getInterpolation(float)' on a null object reference
	at l43.set(r8-map-id-...:84)
	at android.animation.PropertyValuesHolder.setAnimatedValue(PropertyValuesHolder.java:210)
	at android.animation.ObjectAnimator.animateValue(ObjectAnimator.java:703)
	...
	at android.view.View.setVisibility(View.java:1612)
A/android-translation-layer(...): java_vm_ext.cc:563] JNI DETECTED ERROR IN APPLICATION: JNI CallVoidMethod called with pending exception java.lang.NullPointerException
```
Grep: `TimeInterpolator.getInterpolation(float)' on a null object`.
Cause: Material `LinearProgressIndicator` (indeterminate, disjoint mode) loads
`R.anim.linear_indeterminate_line{1,2}_{head,tail}_interpolator` with
`AnimationUtils.loadInterpolator(context, id)` (androidx `AnimationUtilsCompat`
calls the platform method directly on API >= 21) and calls `getInterpolation()` on
the result in its property setter. ATL returns null. Those four files are:
```xml
<pathInterpolator android:controlX1="0.2" android:controlY1="0.0" android:controlX2="0.8" android:controlY2="1.0"/>  <!-- line1_head -->
<pathInterpolator android:controlX1="0.4" android:controlY1="0.0" android:controlX2="1.0" android:controlY2="1.0"/>  <!-- line1_tail -->
<!-- line2_head / line2_tail: same shape, e.g. line2_tail = 0.1, 0.0, 0.45, 1.0 -->
```
So the crash fix needs: XML loading + a real cubic `PathInterpolator`. Same root
cause blocks `android:interpolator` on XML animators (triage "medium 120"), and
every `ValueAnimator` default curve is linear (triage #39). `BounceInterpolator`
is missing entirely (triage #55: ClassNotFoundException crash).

## Current code (what is wrong)
All in `src/api-impl/android/view/animation/`:
- `AnimationUtils.java:12-14` `loadInterpolator(Context, int)` returns `null`.
  No `loadInterpolator(Resources, Theme, int)` overload. `:6` `loadAnimation`
  returns a bare `Animation` (out of scope, see Risks). `:8-10`
  `currentAnimationTimeMillis` uses `System.currentTimeMillis()` (AOSP:
  `SystemClock.uptimeMillis()`; leave it unless you need it).
- `PathInterpolator.java:7-18`: both constructors ignore their arguments,
  `getInterpolation` returns `input`. No `(float,float)` quad constructor, no
  `(Context, AttributeSet)` constructor.
- `AccelerateDecelerateInterpolator.java:6-8` returns `input` (should be a cosine
  curve); does not extend `BaseInterpolator`. It is the default of every
  `ValueAnimator` (`src/api-impl/android/animation/ValueAnimator.java:140-141`).
- `AnticipateInterpolator.java:3-5` and `CycleInterpolator.java:3-5`: not
  interpolators at all (no `implements`, no `getInterpolation`).
- `OvershootInterpolator.java:5`: `implements TimeInterpolator` instead of
  `extends BaseInterpolator` (so it is not an `Interpolator`), no no-arg
  constructor. Formula at `:15` is correct.
- `DecelerateInterpolator.java`: formula OK; missing `(Context, AttributeSet)`.
- `AccelerateInterpolator.java:50-58`: already has a correct XML constructor; copy
  its pattern.
- `BaseInterpolator.java:3-4` is empty; AOSP has hidden
  `getChangingConfiguration()` / `setChangingConfiguration(int)`.
- Missing classes: `BounceInterpolator`, `AnticipateOvershootInterpolator`.
- `src/api-impl/android/animation/AnimatorInflater.java:1054-1061`: the block that
  applies `android:interpolator` is commented out (calls
  `AnimationUtils.loadInterpolator(res, theme, resID)`).
- `src/api-impl/android/graphics/Path.java`: a GskPath builder; there is **no
  `approximate()`**, and `PathMeasure.java` is a stub (`getLength()` returns 100,
  `getPosTan` returns false), so a `Path` cannot be read back in Java.
  Natives are declared at `Path.java:286-306`, implemented in
  `src/api-impl-jni/graphics/android_graphics_Path.c` (see
  `path_foreach_transform` at ~line 129 and `gsk_path_foreach` at line 181).
- SVG path parser: `PathParser` is a package-private class inside
  `src/api-impl/android/graphics/drawable/VectorDrawable.java:54-674`
  (`createPathFromPathData(String)` at :60). `AdaptiveIconDrawable.java:163` uses
  it; both files have a commented `import android.util.PathParser;` (:44 / :46).
- Styleables exist in `src/api-impl/com/android/internal/R.java`:
  `AccelerateInterpolator_factor`, `DecelerateInterpolator_factor`,
  `AnticipateInterpolator_tension`, `AnticipateOvershootInterpolator_tension`
  (0) / `_extraTension` (1), `CycleInterpolator_cycles`,
  `OvershootInterpolator_tension`, `PathInterpolator_controlX1` (0) / `_controlY1`
  (1) / `_controlX2` (2) / `_controlY2` (3) / `_pathData` (4).
- Framework interpolators exist in `res/framework-res/res/interpolator/`
  (`fast_out_slow_in.xml`, `bounce.xml`, ...; 40 of them are `<pathInterpolator>`,
  ~11 use `android:pathData`, e.g. `progress_indeterminate_horizontal_rect1_translatex.xml`:
  `M 0 0 L 0.3665 0 C 0.4725,0.0624 0.6154,0.5 0.6833,0.5 C 0.7548,0.5 0.7573,0.8145 1.0,1.0`).
- `Resources.getAnimation(id)` (`Resources.java:1003`) works for any XML
  resource (the type string is only used in messages), so `res/interpolator` and
  `res/anim` both load. `Theme.obtainStyledAttributes(AttributeSet,int[],int,int)`
  (`Resources.java:1377`) and `Resources.obtainAttributes` (`:1629`) exist.
- New Java files must be added to the explicit list in `src/api-impl/meson.build`
  (animation files at ~line 685, keep alphabetical).

## What Android does
`AnimationUtils.loadInterpolator(Context c, int id)` =
`loadInterpolator(c.getResources(), c.getTheme(), id)`. That opens
`res.getAnimation(id)`, calls `createInterpolatorFromXml(res, theme, parser)`,
closes the parser in `finally`, and wraps `XmlPullParserException`/`IOException`
in `Resources.NotFoundException("Can't load animation resource ID #0x<hex>")`.
`createInterpolatorFromXml`: `depth = parser.getDepth()`; loop
`while (((type = parser.next()) != END_TAG || parser.getDepth() > depth) && type != END_DOCUMENT)`;
skip non-`START_TAG`; `attrs = Xml.asAttributeSet(parser)`; switch on `parser.getName()`:
`linearInterpolator` -> `new LinearInterpolator()`;
`accelerateInterpolator`, `decelerateInterpolator`, `cycleInterpolator`,
`anticipateInterpolator`, `overshootInterpolator`,
`anticipateOvershootInterpolator`, `pathInterpolator` -> `new X(res, theme, attrs)`;
`accelerateDecelerateInterpolator`, `bounceInterpolator` -> `new X()`;
anything else -> `throw new InflateException("Unknown interpolator name: " + name)`.
Returns the last one built. Every XML constructor reads attributes with
`theme != null ? theme.obtainStyledAttributes(attrs, STYLEABLE, 0, 0) : res.obtainAttributes(attrs, STYLEABLE)`
(this resolves `?attr/` theme references) and `recycle()`s. All attributes are in
the `android:` namespace (androidx/Material use the same platform attr IDs; there
is no `app:` variant for interpolator XML).

Formulas (t = input, float math as written):
| class | XML attrs (default) | getInterpolation |
|---|---|---|
| Linear | - | `t` |
| Accelerate | factor (1.0) | factor==1 ? `t*t` : `pow(t, 2*factor)` |
| Decelerate | factor (1.0) | factor==1 ? `1-(1-t)*(1-t)` : `1-pow(1-t, 2*factor)` |
| AccelerateDecelerate | - | `(float)(Math.cos((t + 1) * Math.PI) / 2.0f) + 0.5f` |
| Cycle | cycles (1.0) | `(float)Math.sin(2 * cycles * Math.PI * t)` |
| Anticipate | tension (2.0) | `t*t*((T+1)*t - T)` |
| Overshoot | tension (2.0) | `t -= 1; return t*t*((T+1)*t + T) + 1` |
| AnticipateOvershoot | tension (2.0), extraTension (1.5) | `T = tension*extraTension`; `a(t,s)=t*t*((s+1)*t-s)`, `o(t,s)=t*t*((s+1)*t+s)`; `t<0.5 ? 0.5f*a(2t,T) : 0.5f*(o(2t-2,T)+2)` |
| Bounce | - | `b(t)=t*t*8f`; `t*=1.1226f`; `t<0.3535f ? b(t)` : `t<0.7408f ? b(t-0.54719f)+0.7f` : `t<0.9644f ? b(t-0.8526f)+0.9f` : `b(t-1.0435f)+0.95f` |

Constructors: `AnticipateOvershootInterpolator()` T=2*1.5, `(tension)` T=tension*1.5,
`(tension, extraTension)` T=tension*extraTension. `OvershootInterpolator()` and
`AnticipateInterpolator()` use 2.0f. `CycleInterpolator(float cycles)`.
Public `(Context, AttributeSet)` constructors = `(context.getResources(), context.getTheme(), attrs)`.

`PathInterpolator` (AOSP `PRECISION = 0.002f`): XML: if `pathData` set ->
`PathParser.createPathFromPathData`, null -> `InflateException`; else `controlX1`
and `controlY1` are required (`InflateException("pathInterpolator requires the controlX1 attribute")`),
if exactly one of `controlX2`/`controlY2` is set -> `InflateException("pathInterpolator requires both controlX2 and controlY2 for cubic Beziers.")`;
none -> quad `moveTo(0,0); quadTo(x1,y1,1,1)`; both -> cubic `moveTo(0,0); cubicTo(x1,y1,x2,y2,1,1)`.
`initPath(path)`: `float[] p = path.approximate(PRECISION)` = triples
(fraction-of-length, x, y). Must start at (0,0) and end at (1,1) else
`IllegalArgumentException("The Path must start at (0,0) and end at (1,1)")`;
for each point: same fraction as previous but different x ->
`IllegalArgumentException("The Path cannot have discontinuity in the X axis.")`;
`x < prevX` -> `IllegalArgumentException("The Path cannot loop back on itself.")`;
store `mX[i]=x, mY[i]=y`.
`getInterpolation(t)`: `t<=0 -> 0`, `t>=1 -> 1`; binary search
`start=0,end=mX.length-1; while(end-start>1){mid=(start+end)/2; if(t<mX[mid]) end=mid; else start=mid;}`;
`xRange=mX[end]-mX[start]; if(xRange==0) return mY[start];`
`return mY[start] + (t-mX[start])/xRange * (mY[end]-mY[start]);`

## Implementation steps
One commit per group is fine; build after each step with
`rig build --worktree ~/Develop/android-translation-layer`. dx: **no lambdas or
method references**; anonymous classes only. Keep the AOSP Apache header style of
`AccelerateInterpolator.java` for code copied from AOSP semantics.

**Group A (pure Java, fixes the crash)**
1. `BaseInterpolator.java`: add `private int mChangingConfiguration;`,
   `public int getChangingConfiguration()`, `void setChangingConfiguration(int)`.
2. New helper (package-private, e.g. static method in `AnimationUtils`):
   `static TypedArray obtainAttrs(Resources res, Theme theme, AttributeSet attrs, int[] styleable)`
   doing the theme-or-res choice above. Use it in every XML constructor below.
3. `AccelerateDecelerateInterpolator.java`: `extends BaseInterpolator`, cosine
   formula, add `(Context, AttributeSet)` constructor that ignores attrs.
4. `DecelerateInterpolator.java`: add `(Context, AttributeSet)` and
   `(Resources, Theme, AttributeSet)` reading `DecelerateInterpolator_factor` (1.0f).
   `AccelerateInterpolator.java`: add the `(Resources, Theme, AttributeSet)` one.
5. `CycleInterpolator.java`, `AnticipateInterpolator.java`,
   `OvershootInterpolator.java`: `extends BaseInterpolator`, store the parameter,
   formulas from the table, no-arg constructors where AOSP has them, XML
   constructors. Pitfall: `OvershootInterpolator` must stop implementing
   `TimeInterpolator` directly (BaseInterpolator already does via Interpolator).
6. New `AnticipateOvershootInterpolator.java` and `BounceInterpolator.java`
   (SPDX or Apache header, add to `meson.build`). Use `8.0f`, `1.1226f` etc. exactly.
7. `PathInterpolator.java`: fields `float[] mX, mY`. Add
   `private void initFromPoints(float[] xs, float[] ys)` doing the endpoint and
   loop-back validation above (use a small epsilon, 1e-4f, for the (0,0)/(1,1) check)
   and `getInterpolation` with the binary search. `PathInterpolator(x1,y1,x2,y2)`:
   sample the cubic directly in Java with N=512 segments, `u=i/(float)N`:
   `x = 3*(1-u)*(1-u)*u*x1 + 3*(1-u)*u*u*x2 + u*u*u`, same for y with y1,y2.
   Add `PathInterpolator(float cx, float cy)`: quad
   `x = 2*(1-u)*u*cx + u*u`. Pitfall: do not round `u`; first point must be
   exactly (0,0) and last exactly (1,1) (set them explicitly).
8. `PathInterpolator(Resources, Theme, AttributeSet)` + `(Context, AttributeSet)`:
   `parseInterpolatorFromTypeArray` exactly as above; use `a.hasValue(index)`
   for "set". For `pathData` in Group A: `throw new InflateException("pathInterpolator pathData not supported yet")`
   is **not** acceptable as the final state; do Group B before closing the task.
9. `AnimationUtils.java`: add `loadInterpolator(Resources, Theme, int)` and make
   `loadInterpolator(Context,int)` delegate; add
   `private static Interpolator createInterpolatorFromXml(Resources, Theme, XmlPullParser)`
   with the element switch above (use `if/else` with `name.equals(...)`). Imports:
   `org.xmlpull.v1.XmlPullParser`, `XmlPullParserException`,
   `android.content.res.XmlResourceParser`, `android.util.Xml`,
   `android.view.InflateException`, `java.io.IOException`.

**Group B (paths: pathData and PathInterpolator(Path))**
10. Move `PathParser` out of `VectorDrawable.java` (lines 54-674) into new
    `src/api-impl/android/util/PathParser.java`, `package android.util;`,
    `public class PathParser`, keep `PathDataNode` `public static`; copy the
    imports it needs (`android.graphics.Path`, `android.util.Log`,
    `java.util.ArrayList`, `java.util.Arrays`). Uncomment the imports at
    `VectorDrawable.java:44` and `AdaptiveIconDrawable.java:46`. Add to
    `meson.build`. Only `PathParser` itself touches `PathDataNode.mType`/`mParams`
    (private, fine inside the class); `VectorDrawable` uses only public static
    methods (`deepCopyNodes`, `canMorph`, `PathDataNode.nodesToPath`).
11. C: in `android_graphics_Path.c` add
    `JNIEXPORT jfloatArray JNICALL Java_android_graphics_Path_native_1get_1segments(JNIEnv *env, jclass this, jlong path_ptr)`:
    `gsk_path_foreach(path, GSK_PATH_FOREACH_ALLOW_QUAD | GSK_PATH_FOREACH_ALLOW_CUBIC, cb, garray)`
    (no CONIC flag, so GTK converts conics/arcs to cubics). Callback appends to a
    `GArray` of `float`: `op` (GSK_PATH_MOVE=0, CLOSE, LINE, QUAD, CUBIC: write
    your own codes 0..4), `n_pts`, then `x,y` of each `pts[i]`. Copy into a new
    `jfloatArray` (`(*env)->NewFloatArray`, `SetFloatArrayRegion`), free the GArray.
    Declare `private static native float[] native_get_segments(long path);` in
    `Path.java` after line 306 and add the prototype (signature `(J)[F`, return
    `jfloatArray`) to `src/api-impl-jni/generated_headers/android_graphics_Path.h`
    in the same format as `native_contains`; commit the header.
12. `Path.java`: `public float[] approximate(float acceptableError)`: read
    segments of `getGskPath()`; walk them, emitting points (x,y): MOVE -> point
    pts[0]; LINE/CLOSE -> pts[1]; QUAD/CUBIC -> sample `n = clamp(ceil(controlPolygonLength / (10*acceptableError)), 8, 1000)`
    points, u=k/n, k=1..n. Then compute cumulative chord length per point;
    fraction = length/total (0 if total==0); a MOVE point gets the same fraction as
    the previous point. Return triples `[fraction, x, y]`. No lambdas.
13. `PathInterpolator(Path path)`: `float[] p = path.approximate(0.002f)`; check
    the "discontinuity" rule (same fraction, different x), then
    `initFromPoints`. `pathData` branch of step 8: `PathParser.createPathFromPathData`
    then the same.
14. `AnimatorInflater.java:1054-1061`: uncomment the block (separate commit, it
    changes every XML animator, gate it on its own).

## Failing check to write first
1. Baseline on unfixed main (bazzite):
   `rig atlh --detach ib run --run-id interp-base --force --pkg org.fossify.filemanager org.fossify.musicplayer`,
   then `rig job wait ib`. File Manager must be CRASH and its log
   (`work/runs/interp-base/org.fossify.filemanager.log` on the runner) must contain
   `TimeInterpolator.getInterpolation(float)' on a null object`. Run it twice: same
   result both times (if Music doesn't hit it at startup, it shows when the library
   scan shows the progress bar; File Manager is the reliable one).
2. After the fix the same command with `--run-id interp-fix` must give ALIVE for File
   Manager with no such line, and `interactions/org.fossify.musicplayer.txt` must
   still pass. Check: `grep -c "getInterpolation(float)' on a null" <log>` is 0.
3. Optional extra: a ClassNotFound check for `android.view.animation.BounceInterpolator`
   in `rig atlh missing-apis` output disappears.

## Done when
- `rig build --worktree ~/Develop/android-translation-layer` builds clean (Java + C).
- Check above passes; full gate: `rig atlh --detach gate run --run-id interp-fix`,
  `rig job wait gate`, `rig atlh compare <current baseline> interp-fix` shows 0
  regressions and File Manager CRASH -> ALIVE (rerun flaky regressions with
  `--force --pkg`).
- Commit body: root cause (null from loadInterpolator, Material
  LinearProgressIndicator), evidence (log line), verification. Update this brief's
  status, `docs/roadmap.md` (the NPE family) and `HANDOFF.md`.

## Risks / what not to do
- **No fake fixes.** Returning `new LinearInterpolator()` (or any fixed object) for
  every id, catching the NPE, or a `PathInterpolator` that still returns `input` is a
  fake fix; it hides the curve and the harness then measures nothing.
- Don't swallow XML errors: unknown element -> `InflateException`, bad path ->
  `IllegalArgumentException`, like AOSP. A crash with a clear message is better
  than a silent wrong curve.
- Don't touch `PathMeasure` here (separate gap) and don't build `PathInterpolator`
  on top of it.
- `loadAnimation` / `AnimationSet` XML (`<set>`, `<alpha>`, `<scale>`,
  `<rotate>`, `<translate>`) is out of scope: ATL's `Animation` never applies
  transformations (`Animation.java:40-52` jumps to the end), so parsing XML alone
  changes nothing visible. Separate task (triage "View animations", ~300 lines).
- Fixing `AccelerateDecelerateInterpolator` changes the timing of every
  ValueAnimator; expect screenshot differences mid-animation, not crashes.
- Step 14 can surface new exceptions in apps whose animator XML uses odd
  interpolators: compare carefully and fix the real cause, don't re-comment.
- Nothing goes upstream (AGENTS.md D1).
