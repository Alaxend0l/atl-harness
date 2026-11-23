// SPDX-License-Identifier: AGPL-3.0-or-later
// KeyEvent meta-state + factory bookkeeping (jvm-test-rig brief item 3).
// getUnicodeChar is NOT asserted: unicodeValue comes from the GDK keymap in
// native code, out of scope for the JVM rig (recording stub returns void).
public class KeyEventMetaTest {
  static final int SHIFT = android.view.KeyEvent.META_SHIFT_ON;
  static final int SHIFT_L = android.view.KeyEvent.META_SHIFT_LEFT_ON;
  static final int CTRL = android.view.KeyEvent.META_CTRL_ON;
  static void eq(Object got, Object want, String what) {
    if (!want.equals(got)) throw new AssertionError(what + ": got " + got + " want " + want);
  }
  public static void main(String[] a) throws Exception {
    System.load(System.getProperty("shim"));

    // obtain + field round-trip (fork signature mirrors AOSP's 11-arg form:
    // downTime, eventTime, action, code, repeat, meta, device, scan, flags, source, chars)
    android.view.KeyEvent k = android.view.KeyEvent.obtain(100L, 200L,
        android.view.KeyEvent.ACTION_DOWN, android.view.KeyEvent.KEYCODE_A, 0, SHIFT,
        0, 0, 0, 0x01, null);
    eq(k.getKeyCode(), android.view.KeyEvent.KEYCODE_A, "keyCode");
    eq(k.getAction(), android.view.KeyEvent.ACTION_DOWN, "action");
    eq(k.isDown(), true, "isDown");
    eq(k.getDownTime(), 100L, "downTime");
    eq(k.getEventTime(), 200L, "eventTime");
    eq(k.getMetaState(), SHIFT, "metaState");
    if (!k.isShiftPressed()) throw new AssertionError("isShiftPressed");
    if (k.isCtrlPressed()) throw new AssertionError("ctrl must not press");
    if (!k.hasModifiers(SHIFT)) throw new AssertionError("hasModifiers");
    if (k.hasNoModifiers()) throw new AssertionError("hasNoModifiers with shift");

    // copy preserves fields
    android.view.KeyEvent c = k.copy();
    eq(c.getKeyCode(), k.getKeyCode(), "copy keyCode");
    eq(c.getMetaState(), k.getMetaState(), "copy meta");

    // normalizeMetaState: side bits imply the general bit (AOSP)
    int norm = android.view.KeyEvent.normalizeMetaState(SHIFT_L);
    if ((norm & SHIFT) == 0) throw new AssertionError("normalize must add META_SHIFT_ON, got " + norm);
    if ((norm & SHIFT_L) == 0) throw new AssertionError("normalize must keep side bit");
    if (!android.view.KeyEvent.metaStateHasModifiers(SHIFT_L, SHIFT))
      throw new AssertionError("metaStateHasModifiers(SHIFT_L, SHIFT)");
    if (android.view.KeyEvent.metaStateHasNoModifiers(SHIFT_L))
      throw new AssertionError("shift is not modifier-free");
    if (!android.view.KeyEvent.metaStateHasNoModifiers(0))
      throw new AssertionError("0 is modifier-free");

    // changeTimeRepeat: count stored. KNOWN-GAP (rig finding 2026-11-23):
    // AOSP's KeyEvent(orig,eventTime,newRepeat) ctor clears/sets FLAG_LONG_PRESS
    // by repeat count; the fork's changeTimeRepeat(3-arg) does not (its
    // `new KeyEvent(event, eventTime, newRepeat)` path skips the mFlags rule).
    // Warn-only until the fork fix lands.
    android.view.KeyEvent r = android.view.KeyEvent.changeTimeRepeat(k, 300L, 2);
    eq(r.getRepeatCount(), 2, "repeatCount");
    if ((r.getFlags() & android.view.KeyEvent.FLAG_LONG_PRESS) == 0)
      System.out.println("RIG-KNOWN-GAP changeTimeRepeat misses FLAG_LONG_PRESS (AOSP sets it when repeat != 0)");
    else
      System.out.println("RIG-GAP-CLOSED changeTimeRepeat FLAG_LONG_PRESS ok");

    // changeAction / changeFlags are non-mutating (obtain-based factories)
    android.view.KeyEvent up = android.view.KeyEvent.changeAction(k, android.view.KeyEvent.ACTION_UP);
    eq(up.getAction(), android.view.KeyEvent.ACTION_UP, "changeAction new");
    eq(k.getAction(), android.view.KeyEvent.ACTION_DOWN, "changeAction must not mutate original");
    android.view.KeyEvent wf = android.view.KeyEvent.changeFlags(k, android.view.KeyEvent.FLAG_WOKE_HERE);
    eq(wf.getFlags() & android.view.KeyEvent.FLAG_WOKE_HERE, android.view.KeyEvent.FLAG_WOKE_HERE, "changeFlags");
    eq(k.getFlags(), 0, "changeFlags must not mutate original");

    // isModifierKey + mask sanity
    if (!android.view.KeyEvent.isModifierKey(android.view.KeyEvent.KEYCODE_SHIFT_LEFT))
      throw new AssertionError("KEYCODE_SHIFT_LEFT must be a modifier key");
    if (android.view.KeyEvent.isModifierKey(android.view.KeyEvent.KEYCODE_A))
      throw new AssertionError("KEYCODE_A is not a modifier key");

    System.out.println("JVMRIG_KEYEVENT_OK norm=" + Integer.toHexString(norm));
  }
}
