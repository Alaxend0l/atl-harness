# SPDX-License-Identifier: AGPL-3.0-or-later
"""scripts/atl-gaps: classification of ATL method bodies (no network, no ATL)."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "atl-gaps"

import importlib.machinery  # noqa: E402
import importlib.util  # noqa: E402

_loader = importlib.machinery.SourceFileLoader("atl_gaps", str(SCRIPT))
_spec = importlib.util.spec_from_loader("atl_gaps", _loader)
atl_gaps = importlib.util.module_from_spec(_spec)
_loader.exec_module(atl_gaps)

JAVA = '''package android.widget;

public class Foo {
    private int count;

    public void doNothing() {
    }

    public void removedImpl() {
        // count = 1;
        // notifyAll();
    }

    public boolean isFlag() {
        return false;
    }

    public String name() {
        return "x";   // not a constant return: real value
    }

    public void onlyLogs(int x) {
        Log.d("tag", "x=" + x);
    }

    public int logAndConst() {
        System.out.println("here");
        return 0;
    }

    public void nope() {
        throw new UnsupportedOperationException("nope");
    }

    public void halfDone(int x) {
        count = x;
        // TODO: also notify observers
    }

    public void noisy(int x) {
        System.out.println("got " + x);
        count = x;
    }

    public void bracesInString() {
        Log.d("tag", "} return null; {");
        count = 1;
    }

    public void callsStub() {
        stubs("no-op");
    }

    public int getCount() {
        return count;
    }

    public native void nativeThing();
}
'''


class Scan(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        (root / "src" / "api-impl" / "android" / "widget").mkdir(parents=True)
        (root / "src" / "api-impl-jni").mkdir(parents=True)
        (root / "src" / "api-impl" / "android" / "widget" / "Foo.java").write_text(JAVA)
        (root / "src" / "api-impl-jni" / "Thing.c").write_text(
            'static const JNINativeMethod m[] = {{"nativeThing", "()V", (void*)impl}};')
        self.root = root
        self.members = {m.name: m for m in atl_gaps.scan_atl(root)[0]}

    def tearDown(self):
        self.tmp.cleanup()

    def test_kinds(self):
        want = {
            "doNothing": "empty",
            "removedImpl": "commented-out",
            "isFlag": "const-return",
            "onlyLogs": "log-only",
            "logAndConst": "log+const",
            "nope": "unsupported",
            "halfDone": "todo",
            "noisy": "print-in-body",
            "callsStub": "stub-call",
            "nativeThing": "implemented",   # symbol is in the .c table
        }
        for name, kind in want.items():
            self.assertIn(name, self.members, f"{name} not scanned")
            self.assertEqual(self.members[name].kind, kind, f"{name}: {self.members[name].note}")

    def test_string_and_comment_masking(self):
        # `}` and `return null;` inside a string literal must not end the body or
        # look like a statement
        m = self.members["bracesInString"]
        self.assertEqual(m.kind, "print-in-body")
        self.assertIn("Log.d", m.note)
        # the method after it is still scanned with a correct kind
        self.assertEqual(self.members["getCount"].kind, "implemented")
        self.assertEqual(self.members["isFlag"].kind, "const-return")

    def test_class_attribution(self):
        self.assertEqual(self.members["nope"].clazz, "android.widget.Foo")
        self.assertEqual(self.members["nope"].file, "src/api-impl/android/widget/Foo.java")
        self.assertGreater(self.members["nope"].line, 1)

    def test_native_missing_without_symbol(self):
        (self.root / "src" / "api-impl-jni" / "Thing.c").write_text("/* nothing */")
        members = {m.name: m for m in atl_gaps.scan_atl(self.root)[0]}
        self.assertEqual(members["nativeThing"].kind, "native-missing")

    def test_queue_item_is_canonical(self):
        item = atl_gaps.queue_item(self.members["nope"], {}, {}, {})
        self.assertEqual(list(item), atl_gaps.CANON)
        self.assertIn("UnsupportedOperationException", item["title"])
        self.assertEqual(item["category"], "unimplemented_method")


class Cli(unittest.TestCase):
    def test_json_and_emit_queue(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            (d / "atl" / "src" / "api-impl" / "android" / "widget").mkdir(parents=True)
            (d / "atl" / "src" / "api-impl" / "android" / "widget" / "Foo.java").write_text(JAVA)
            (d / "atl" / "src" / "api-impl-jni").mkdir(parents=True)
            q = d / "queue.json"
            q.write_text(json.dumps({"updated": "x", "items": []}))
            p = subprocess.run([sys.executable, str(SCRIPT), "--atl", str(d / "atl"),
                                "--queue", str(q), "--json"], capture_output=True, text=True)
            self.assertEqual(p.returncode, 0, p.stderr)
            rows = json.loads(p.stdout)
            self.assertTrue(any(r["kind"] == "empty" and r["name"] == "doNothing" for r in rows))
            p2 = subprocess.run([sys.executable, str(SCRIPT), "--atl", str(d / "atl"),
                                 "--queue", str(q), "--emit-queue", "3"], capture_output=True, text=True)
            self.assertEqual(p2.returncode, 0, p2.stderr)
            items = json.loads(q.read_text())["items"]
            self.assertEqual(len(items), 3)
            self.assertEqual(list(items[0]), atl_gaps.CANON)

    def test_bad_root_exits_2(self):
        p = subprocess.run([sys.executable, str(SCRIPT), "--atl", "/nonexistent"],
                           capture_output=True, text=True)
        self.assertEqual(p.returncode, 2)


if __name__ == "__main__":
    unittest.main()
