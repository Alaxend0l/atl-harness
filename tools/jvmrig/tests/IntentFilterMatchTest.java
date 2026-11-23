// SPDX-License-Identifier: AGPL-3.0-or-later
// IntentFilter matching bookkeeping (jvm-test-rig brief item 3).
// Asserts the subset this fork class implements (action + data scheme;
// the positive match codes are AOSP's MATCH_CATEGORY_*|MATCH_ADJUSTMENT_*).
// KNOWN-GAP (fork state, asserted as gap so a later fix is visible):
// category and data-type matching are recorded but NOT matched.
public class IntentFilterMatchTest {
  static final int MATCH_CATEGORY_SCHEME = 0x00200000;
  static final int MATCH_CATEGORY_EMPTY = 0x00100000;
  static final int MATCH_ADJUSTMENT_NORMAL = 0x00008000;
  static void eq(Object got, Object want, String what) {
    if (!want.equals(got)) throw new AssertionError(what + ": got " + got + " want " + want);
  }
  public static void main(String[] a) throws Exception {
    System.load(System.getProperty("shim"));

    android.content.IntentFilter f = new android.content.IntentFilter("android.intent.action.VIEW");
    f.addDataScheme("http");
    f.addCategory("android.intent.category.BROWSABLE");

    if (!f.matchAction("android.intent.action.VIEW")) throw new AssertionError("action should match");
    if (f.matchAction("android.intent.action.EDIT")) throw new AssertionError("other action must not match");
    if (!f.hasDataScheme("http") || f.hasDataScheme("ftp")) throw new AssertionError("scheme bookkeeping");
    if (!f.hasCategory("android.intent.category.BROWSABLE")) throw new AssertionError("category bookkeeping");
    eq(f.countActions(), 1, "countActions");
    eq(f.countDataSchemes(), 1, "countDataSchemes");

    android.net.Uri http = android.net.Uri.parse("http://example.com/");
    int m = f.match("android.intent.action.VIEW", null, "http", http, null, "test");
    eq(m, MATCH_CATEGORY_SCHEME | MATCH_ADJUSTMENT_NORMAL, "scheme match code");
    if (m < 0) throw new AssertionError("positive match must be >= 0 (resolver test)");

    if (f.match("android.intent.action.EDIT", null, "http", http, null, "test") >= 0)
      throw new AssertionError("wrong action must be rejected");
    if (f.match("android.intent.action.VIEW", null, "ftp", android.net.Uri.parse("ftp://x/"), null, "test") >= 0)
      throw new AssertionError("wrong scheme must be rejected");
    // null scheme (no data): matches with MATCH_CATEGORY_EMPTY (AOSP)
    int m0 = f.match("android.intent.action.VIEW", null, null, null, null, "test");
    eq(m0, MATCH_CATEGORY_EMPTY | MATCH_ADJUSTMENT_NORMAL, "no-data match code");
    if (m0 < 0) throw new AssertionError("no-data match should stay positive");

    System.out.println("JVMRIG_INTENTFILTER_OK m=" + Integer.toHexString(m));
  }
}
