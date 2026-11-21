# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests for scripts/agent-loop's diff lint (no network, no opencode)."""
import importlib.machinery
import importlib.util
import types
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
_loader = importlib.machinery.SourceFileLoader("agent_loop", str(REPO / "scripts" / "agent-loop"))
_spec = importlib.util.spec_from_loader("agent_loop", _loader)
al = importlib.util.module_from_spec(_spec)
_loader.exec_module(al)


def lint(diff, max_removed=150):
    loop = al.Loop.__new__(al.Loop)
    loop.a = types.SimpleNamespace(max_removed=max_removed)
    return loop.lint(diff)


HEAD = "diff --git a/src/api-impl/android/widget/X.java b/src/api-impl/android/widget/X.java\n"


class Lint(unittest.TestCase):
    def test_clean_change_passes(self):
        reject, flags = lint(HEAD + "+\tpublic int getFoo() {\n+\t\treturn foo;\n+\t}\n")
        self.assertEqual((reject, flags), ([], []))

    def test_catch_alls_rejected(self):
        for line in ("+\t} catch (Exception e) {", "+\t} catch (final Throwable t) {",
                     "+\tcatch (java.lang.RuntimeException e) {", "+\ttry { x(); } catch (Foo e) {}"):
            with self.subTest(line):
                reject, _ = lint(HEAD + line + "\n")
                self.assertTrue(reject, line)

    def test_specific_catch_allowed(self):
        reject, _ = lint(HEAD + "+\t} catch (IOException e) {\n+\t\tLog.e(TAG, \"x\", e);\n")
        self.assertEqual(reject, [])

    def test_outside_src_rejected(self):
        reject, _ = lint("diff --git a/meson.build b/meson.build\n+foo\n")
        self.assertTrue(any("outside src/" in r for r in reject))

    def test_mass_deletion_rejected(self):
        reject, _ = lint(HEAD + "-x\n" * 200, max_removed=150)
        self.assertTrue(any("removes 200" in r for r in reject))

    def test_suspicious_returns_flagged_not_rejected(self):
        reject, flags = lint(HEAD + "+\t\treturn null;\n+\t\treturn 0;\n")
        self.assertEqual(reject, [])
        self.assertEqual(len(flags), 2)


if __name__ == "__main__":
    unittest.main()


class Queue(unittest.TestCase):
    def loop(self, queue):
        loop = al.Loop.__new__(al.Loop)
        loop.a = types.SimpleNamespace(queue=queue)
        loop.log = lambda *_: None
        return loop

    def test_triage_queue_builds_prompts(self):
        items = self.loop("triage").queue()
        self.assertTrue(items)
        fixed = [ln.strip().lower() for ln in al.SKIP_FILE.read_text().splitlines()
                 if ln.strip() and not ln.startswith("#")]
        for it in items:
            with self.subTest(it["title"][:40]):
                self.assertFalse(any(f in it["title"].lower() for f in fixed))
                p = self.loop("triage").prompt_for(it)
                self.assertIn(it["title"], p)
                if it.get("fix_hint"):
                    self.assertIn(it["fix_hint"], p)

    def test_audit_queue_still_works(self):
        items = self.loop("audit").queue()
        self.assertGreater(len(items), 100)
        self.loop("audit").prompt_for(items[0])


class Review(unittest.TestCase):
    def test_review_prompt_carries_hint_and_diff(self):
        loop = al.Loop.__new__(al.Loop)
        it = {"title": "X drops Y", "file": "src/a.java", "fix_hint": "call z() first"}
        p = loop.review_prompt(it, "+\tz();\n")
        for part in ("X drops Y", "call z() first", "+\tz();", "OK, FIXED"):
            self.assertIn(part, p)
