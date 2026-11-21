# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests for scripts/runner/builderrors.py (no network, no toolchains)."""
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "runner" / "builderrors.py"
_spec = importlib.util.spec_from_file_location("builderrors", SCRIPT)
be = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(be)


def kinds(text):
    issues, warnings = be.parse(text)
    return [(i.kind, i.where(), i.message) for i in issues], warnings


class Parse(unittest.TestCase):
    def test_c_error_with_context_and_warnings(self):
        log = """[12/80] Compiling C object src/libfoo.so.p/a.c.o
../src/api-impl-jni/audio/android_media_SoundPool.c:30:2: warning: unused variable 'x' [-Wunused-variable]
../src/api-impl-jni/audio/android_media_SoundPool.c:31:9: error: 'GtkMediaStreamX' undeclared (first use in this function)
   31 |         GtkMediaStreamX *s;
      |         ^~~~~~~~~~~~~~~
FAILED: src/libfoo.so.p/a.c.o
ninja: build stopped: subcommand failed.
"""
        issues, warnings = be.parse(log)
        self.assertEqual(warnings, 1)
        self.assertEqual(len(issues), 1, "ninja FAILED is dropped when a specific error exists")
        i = issues[0]
        self.assertEqual((i.kind, i.line), ("c", 31))
        self.assertTrue(i.file.endswith("android_media_SoundPool.c"))
        self.assertIn("undeclared", i.message)
        self.assertIn("GtkMediaStreamX *s;", i.context[0])

    def test_javac(self):
        got, _ = kinds("src/api-impl/android/media/SoundPool.java:60: error: cannot find symbol\n  symbol: class Lopper\n")
        self.assertEqual(got, [("java", "src/api-impl/android/media/SoundPool.java:60", "cannot find symbol")])

    def test_link_errors(self):
        log = ("/usr/bin/ld: main.o: in function `f':\nmain.c:(.text+0x1): undefined reference to `gtk_foo'\n"
               "/usr/bin/ld: cannot find -lvixl: No such file or directory\ncollect2: error: ld returned 1 exit status\n")
        got, _ = kinds(log)
        self.assertIn(("link", "", "undefined reference to `gtk_foo'"), got)
        self.assertTrue(any(k == "link" and "cannot find -lvixl" in m for k, _, m in got))

    def test_meson_and_pkgconfig(self):
        log = ("Run-time dependency libswscale found: NO (tried pkg-config)\n"
               "meson.build:245:92: ERROR: Dependency \"libswscale\" not found, tried pkgconfig\n")
        got, _ = kinds(log)
        self.assertEqual(got[0][0], "meson")
        self.assertEqual(got[0][1], "meson.build:245")
        self.assertIn("libswscale", got[0][2])

    def test_cargo_location(self):
        log = "error[E0425]: cannot find value `x` in this scope\n  --> src/runner.rs:12:5\n   |\nerror: could not compile `atlh`\n"
        got, _ = kinds(log)
        self.assertEqual(got, [("cargo", "src/runner.rs:12", "cannot find value `x` in this scope")])

    def test_make_only_fallback_and_dedup(self):
        log = "make: *** [build/core/binary.mk:707: out/x.o] Error 1\nmake: *** [build/core/binary.mk:707: out/x.o] Error 1\n"
        got, _ = kinds(log)
        self.assertEqual(len(got), 1)
        self.assertEqual(got[0][0], "make")

    def test_dnf_and_clean_log(self):
        self.assertEqual(kinds("No match for argument: mesa-utils\n")[0][0][0], "pkg")
        self.assertEqual(kinds("[1/1] Linking target foo\nok\n"), ([], 0))

    def test_ansi_stripped(self):
        got, _ = kinds("\x1b[1mfoo.c:3:1: \x1b[31merror: \x1b[0mexpected ';'\n")
        self.assertEqual(got[0][:2], ("c", "foo.c:3"))


class Cli(unittest.TestCase):
    def test_json_and_exit_codes(self):
        with tempfile.NamedTemporaryFile("w", suffix=".log", delete=False) as f:
            f.write("a.c:1:1: error: boom\n")
        p = subprocess.run([sys.executable, str(SCRIPT), f.name, "--json"], capture_output=True, text=True)
        self.assertEqual(p.returncode, 1)
        self.assertEqual(json.loads(p.stdout)["errors"][0]["message"], "boom")
        with open(f.name, "w") as g:
            g.write("all good\n")
        p = subprocess.run([sys.executable, str(SCRIPT), f.name], capture_output=True, text=True)
        self.assertEqual(p.returncode, 0)
        self.assertIn("0 error(s)", p.stdout)


if __name__ == "__main__":
    unittest.main()
