// SPDX-License-Identifier: AGPL-3.0-or-later
// SpannableStringBuilder gap + span-shift management, deeper than the v0
// smoke test (jvm-test-rig brief item 3). Expected semantics: AOSP.
public class SpannedGapTest {
  static final int EX = android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE;
  static final int IN = android.text.Spanned.SPAN_INCLUSIVE_INCLUSIVE;
  static final int IX = android.text.Spanned.SPAN_INCLUSIVE_EXCLUSIVE;
  static void eq(Object got, Object want, String what) {
    if (!want.equals(got)) throw new AssertionError(what + ": got " + got + " want " + want);
  }
  public static void main(String[] a) throws Exception {
    System.load(System.getProperty("shim"));

    android.text.SpannableStringBuilder sb = new android.text.SpannableStringBuilder("abcdef");
    Object sp = new Object();
    sb.setSpan(sp, 1, 4, EX);            // covers "bcde"
    eq(sb.getSpanStart(sp), 1, "initial start");
    eq(sb.getSpanEnd(sp), 4, "initial end");
    eq(sb.getSpanFlags(sp), EX, "flags");

    // insertion before the span shifts both edges
    sb.insert(0, "X");                  // Xabcdef
    eq(sb.getSpanStart(sp), 2, "start after left insert");
    eq(sb.getSpanEnd(sp), 5, "end after left insert");

    // EXCLUSIVE start: text inserted exactly at start lands OUTSIDE
    sb.insert(2, "Y");                  // XYabcdef
    eq(sb.getSpanStart(sp), 3, "exclusive start keeps inserted text out");
    // EXCLUSIVE end: insert at end lands OUTSIDE (end unchanged)
    sb.insert(sb.getSpanEnd(sp), "Z");
    eq(sb.getSpanEnd(sp), 6, "exclusive end stays");

    // INCLUSIVE_INCLUSIVE span: both edges absorb boundary inserts
    Object sp2 = new Object();
    int base = sb.length();
    sb.append("qrst");
    sb.setSpan(sp2, base, base + 4, IN);
    sb.insert(base, "!");
    eq(sb.getSpanStart(sp2), base, "inclusive start absorbs");
    eq(sb.toString().substring(sb.getSpanStart(sp2), sb.getSpanEnd(sp2)), "!qrst", "inclusive contents");

    // collapsing an EXCLUSIVE_EXCLUSIVE span by deleting its text removes it
    // (AOSP: a zero-length EX_EX span is dropped)
    android.text.SpannableStringBuilder sb2 = new android.text.SpannableStringBuilder("abc");
    Object sp4 = new Object();
    sb2.setSpan(sp4, 1, 2, EX);
    sb2.delete(1, 2);
    eq(sb2.getSpanStart(sp4), -1, "collapsed EX_EX span dropped");

    // INCLUSIVE_EXCLUSIVE: left boundary absorbs, right does not
    android.text.SpannableStringBuilder sb3 = new android.text.SpannableStringBuilder("abc");
    Object sp5 = new Object();
    sb3.setSpan(sp5, 1, 2, IX);
    sb3.insert(1, "Q");                 // at start: inclusive -> span starts before Q
    eq(sb3.getSpanStart(sp5), 1, "IX start stays and absorbs");
    eq(sb3.toString().substring(1, 2), "Q", "absorbed char");

    System.out.println("JVMRIG_SPAN_OK len=" + sb.length());
  }
  static int countWithout(android.text.SpannableStringBuilder sb, Object sp) {
    int n = 0;
    for (Object o : sb.getSpans(0, sb.length(), Object.class)) if (o != sp) n++;
    return n;
  }
}
