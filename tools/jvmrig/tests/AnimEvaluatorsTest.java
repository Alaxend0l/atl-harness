// SPDX-License-Identifier: AGPL-3.0-or-later
// Animation evaluators (jvm-test-rig brief item 3). ArgbEvaluator is
// exercised in AOSP RGB mode (one endpoint has alpha < 255, where AOSP's
// HSV shortcut does not apply), the linear formula both share.
public class AnimEvaluatorsTest {
  static void eq(Object got, Object want, String what) {
    if (!want.equals(got)) throw new AssertionError(what + ": got " + Integer.toHexString((Integer) got) + " want " + Integer.toHexString((Integer) want));
  }
  public static void main(String[] a) throws Exception {
    System.load(System.getProperty("shim"));

    android.animation.ArgbEvaluator argb = android.animation.ArgbEvaluator.getInstance();
    if (argb == null) throw new AssertionError("getInstance() returned null (queue item: Theme/Argb singleton)");

    // black (opaque) -> semi-transparent red; RGB-interpolated midpoint
    int start = 0xFF000000, end = 0x80FF0000;
    eq(argb.evaluate(0f, start, end), start, "argb frac 0");
    eq(argb.evaluate(1f, start, end), end, "argb frac 1");
    eq(argb.evaluate(0.5f, start, end), 0xC0800000, "argb midpoint");

    android.animation.FloatEvaluator fl = new android.animation.FloatEvaluator();
    eq(fl.evaluate(0.5f, 0f, 10f), 5f, "float mid");
    eq(fl.evaluate(0.25f, 4f, 8f), 5f, "float quarter");

    System.out.println("JVMRIG_ANIM_OK mid=" + Integer.toHexString(argb.evaluate(0.5f, start, end)));
  }
}
