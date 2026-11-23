// SPDX-License-Identifier: AGPL-3.0-or-later
// Uri semantics beyond the v0 smoke test (docs/agent-tasks/jvm-test-rig.md item 3).
public class UriMoreTest {
  static void eq(Object got, Object want, String what) {
    if (!java.util.Objects.equals(got, want)) throw new AssertionError(what + ": got " + got + " want " + want);
  }
  public static void main(String[] a) throws Exception {
    System.load(System.getProperty("shim"));

    android.net.Uri u = android.net.Uri.parse("https://example.com:8080/a/b/c?q=1&r=2#frag");
    eq(u.getScheme(), "https", "scheme");
    eq(u.getHost(), "example.com", "host");
    eq(u.getPort(), 8080, "port");
    eq(u.getPath(), "/a/b/c", "path");
    eq(u.getPathSegments().size(), 3, "segments");
    eq(u.getPathSegments().get(1), "b", "segment 1");
    eq(u.getQuery(), "q=1&r=2", "query");
    eq(u.getFragment(), "frag", "fragment");
    eq(u.getQueryParameter("r"), "2", "query param r");
    eq(u.getQueryParameter("zz"), null, "absent query param");

    // opaque (scheme-specific part, no path separator)
    android.net.Uri m = android.net.Uri.parse("mailto:nobody@example.com");
    if (!m.isOpaque()) throw new AssertionError("mailto should be opaque");
    eq(m.getSchemeSpecificPart(), "nobody@example.com", "ssp");

    // Builder round-trip
    android.net.Uri b = new android.net.Uri.Builder()
        .scheme("https").authority("example.com").path("/a/b")
        .appendQueryParameter("x", "1").build();
    eq(b.toString(), "https://example.com/a/b?x=1", "builder toString");

    // encode/decode: default encode escapes '/'
    eq(android.net.Uri.encode("a b/c"), "a%20b%2Fc", "encode");
    eq(android.net.Uri.decode("a%20b%2Fc"), "a b/c", "decode");

    // no-query Uri: getQueryParameter must be null, not crash
    eq(android.net.Uri.parse("file:///x").getQueryParameter("q"), null, "param on queryless uri");

    System.out.println("JVMRIG_URI_OK host=" + u.getHost() + " port=" + u.getPort());
  }
}
