// SPDX-License-Identifier: AGPL-3.0-or-later
// Headless smoke test of the JVM rig (docs/agent-tasks/jvm-test-rig.md):
// pure-logic framework classes must run on a stock JVM with no ART, GTK or
// Xvfb. Convention: a `main` that throws AssertionError on failure; the
// runner reports PASS/FAIL per class.
public class TextAndUriTest {
  public static void main(String[] a) throws Exception {
    System.load(System.getProperty("shim"));

    // android.text: the span engine is pure Java; historically a rich seam
    // for compat bugs (marquee, Editable semantics).
    android.text.SpannableStringBuilder sb = new android.text.SpannableStringBuilder("hello world");
    sb.replace(0, 5, "HELLO");
    if (!sb.toString().equals("HELLO world")) throw new AssertionError("replace: " + sb);
    sb.setSpan(new android.text.style.StyleSpan(1), 0, 5, android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
    android.text.style.StyleSpan[] sp = sb.getSpans(0, 5, android.text.style.StyleSpan.class);
    if (sp.length != 1 || sb.getSpanStart(sp[0]) != 0 || sb.getSpanEnd(sp[0]) != 5)
      throw new AssertionError("span bookkeeping wrong: " + sp.length);

    // android.net.Uri: parsing/authority/path semantics used by intents.
    android.net.Uri u = android.net.Uri.parse("content://media/external/audio/media/42?x=1");
    if (!"42".equals(u.getLastPathSegment())) throw new AssertionError("uri: " + u);
    if (!"media".equals(u.getAuthority())) throw new AssertionError("authority: " + u);
    if (!"1".equals(u.getQueryParameter("x"))) throw new AssertionError("query: " + u);

    System.out.println("JVMRIG_SMOKE_OK spans=" + sp.length + " uri=" + u.getLastPathSegment());
  }
}
