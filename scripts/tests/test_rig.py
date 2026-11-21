# SPDX-License-Identifier: AGPL-3.0-or-later
"""Unit tests for scripts/rig. No network, ssh or runners needed.

Run: python3 -m unittest discover -s scripts/tests
"""
import importlib.machinery
import importlib.util
import os
import shlex
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
_loader = importlib.machinery.SourceFileLoader("rig", str(REPO / "scripts" / "rig"))
_spec = importlib.util.spec_from_loader("rig", _loader)
rig = importlib.util.module_from_spec(_spec)
_loader.exec_module(rig)

GOOD = """
default = "a"
[runners.a]
ssh = "host-a"
arch = "x86_64"
distrobox = "atl"
[runners.b]
ssh = "host-b"
arch = "aarch64"
config = "config.aarch64.toml"
jobs = 3
lima_host = "mac"
lima_instance = "atl"
"""

AWKWARD = ["plain", "with space", "it's", 'dq"x', "$HOME", "a;b", "", "*"]


class Registry(unittest.TestCase):
    def test_parse(self):
        reg = rig.parse_registry(GOOD)
        self.assertEqual(reg.default, "a")
        self.assertEqual(reg.runners["a"].distrobox, "atl")
        self.assertEqual(reg.runners["a"].config, "config.toml")
        self.assertEqual(reg.runners["b"].jobs, 3)

    def test_rejects(self):
        cases = {
            "unknown key": '[runners.a]\nssh="h"\narch="x86_64"\nbogus=1\n',
            "missing ssh": '[runners.a]\narch="x86_64"\n',
            "lima pair": '[runners.a]\nssh="h"\narch="x"\nlima_host="m"\n',
            "bad default": 'default="zz"\n[runners.a]\nssh="h"\narch="x"\n',
            "top-level key": 'foo=1\n',
            "bad name": '[runners."a b"]\nssh="h"\narch="x"\n',
        }
        for what, text in cases.items():
            with self.subTest(what), self.assertRaises(rig.RigError):
                rig.parse_registry(text)

    def test_pick_runner(self):
        reg = rig.parse_registry(GOOD)
        old = os.environ.pop("ATLH_RUNNER", None)
        try:
            self.assertEqual(rig.pick_runner(reg, None).name, "a")
            self.assertEqual(rig.pick_runner(reg, "b").name, "b")
            os.environ["ATLH_RUNNER"] = "b"
            self.assertEqual(rig.pick_runner(reg, None).name, "b")
            with self.assertRaises(rig.RigError):
                rig.pick_runner(reg, "nope")
            with self.assertRaises(rig.RigError):
                rig.pick_runner(rig.Registry(), None)
        finally:
            os.environ.pop("ATLH_RUNNER", None)
            if old is not None:
                os.environ["ATLH_RUNNER"] = old

    def test_toml_roundtrip(self):
        r = rig.Runner(name="new", ssh="h", arch="aarch64", config="config.aarch64.toml", jobs=2,
                       lima_host="mac", lima_instance="vm", notes='say "hi"')
        back = rig.parse_registry(rig.runner_toml(r)).runners["new"]
        self.assertEqual(back, r)


class Quoting(unittest.TestCase):
    def run_inner(self, script, args, env=None):
        r = rig.Runner(name="t", ssh="x", arch="x86_64")
        inner = rig.inner_script(r, script, args, cd=False, env=env)
        return subprocess.run(["bash", "-c", inner], capture_output=True, text=True, check=True).stdout

    def test_args_survive(self):
        out = self.run_inner('printf "%s\\n" "$@"', AWKWARD)
        self.assertEqual(out.split("\n")[:-1], AWKWARD)

    def test_env_values_are_shell_expressions(self):
        out = self.run_inner('echo "$A|$B"', [], env={"A": shlex.quote("x y"), "B": '"$HOME"/d'})
        self.assertEqual(out.strip(), f"x y|{os.environ['HOME']}/d")

    def test_wrap_distrobox(self):
        r = rig.Runner(name="t", ssh="x", arch="x86_64", distrobox="my box")
        argv = shlex.split(rig.wrap(r, "echo 'q' \"$X\""))
        self.assertEqual(argv[:5], ["distrobox", "enter", "my box", "--", "bash"])
        self.assertEqual(argv[5:], ["-lc", "echo 'q' \"$X\""])

    def test_home_path(self):
        r = rig.Runner(name="t", ssh="x", arch="x")
        self.assertEqual(r.home_path("/abs/p"), "/abs/p")
        out = subprocess.run(["bash", "-c", "echo " + r.home_path("d i r")], capture_output=True, text=True).stdout
        self.assertEqual(out.strip(), os.environ["HOME"] + "/d i r")


class RepoHash(unittest.TestCase):
    def test_matches_shell_pipeline(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "src" / "sub dir").mkdir(parents=True)
            (root / "src" / "main.rs").write_text("fn main() {}\n")
            (root / "src" / "sub dir" / "a b.rs").write_text("x")
            (root / "src" / "Z.rs").write_text("y")
            (root / "Cargo.toml").write_text("[package]\n")
            (root / "config.toml").write_text("a=1\n")
            (root / "config.aarch64.toml").write_text("a=2\n")
            # Same pipeline as scripts/runner/doctor.sh.
            sh = ("{ find src -type f; ls Cargo.toml Cargo.lock config*.toml 2>/dev/null; } "
                  "| LC_ALL=C sort | xargs -d '\\n' sha256sum | sha256sum | cut -d' ' -f1")
            want = subprocess.run(["bash", "-c", sh], cwd=root, capture_output=True, text=True).stdout.strip()
            self.assertEqual(rig.local_repo_hash(root), want)


class Jobs(unittest.TestCase):
    def test_names(self):
        rig.check_job_name("census-1.b_2")
        for bad in ["", "-x", "a b", "a/b", "$(x)"]:
            with self.subTest(bad), self.assertRaises(rig.RigError):
                rig.check_job_name(bad)

    def test_lifecycle_locally(self):
        with tempfile.TemporaryDirectory() as home:
            env = {**os.environ, "HOME": home}
            start = rig.job_start_script("t1", 'echo "in $PWD"; exit 7', '"$HOME"')
            p = subprocess.run(["bash", "-c", start], env=env, capture_output=True, text=True)
            self.assertEqual(p.returncode, 0, p.stderr)
            exit_file = Path(home, "atlh-jobs", "t1.exit")
            for _ in range(50):
                if exit_file.exists() and exit_file.read_text().strip():
                    break
                time.sleep(0.1)
            self.assertEqual(exit_file.read_text().strip(), "7")
            self.assertEqual(Path(home, "atlh-jobs", "t1.log").read_text().strip(), f"in {home}")
            status = subprocess.run(["bash", "-c", rig.JOB_STATUS_SCRIPT], env=env,
                                    capture_output=True, text=True).stdout
            self.assertIn("exit 7", status)


class Cli(unittest.TestCase):
    def parse(self, *argv):
        return rig.build_parser().parse_args(list(argv))

    def test_runner_flag_either_side(self):
        self.assertEqual(self.parse("-r", "mini", "list").runner, "mini")
        self.assertEqual(self.parse("list", "-r", "mini").runner, "mini")

    def test_smoke_options_after_apk(self):
        a = self.parse("smoke", "x.apk", "--settle", "5", "--", "-l", "a/B")
        self.assertEqual((a.apk, a.settle, a.atl_args), ("x.apk", 5, ["-l", "a/B"]))

    def test_atlh_passthrough(self):
        a = self.parse("atlh", "--detach", "j", "run", "--run-id", "x", "--force")
        self.assertEqual(a.detach, "j")
        self.assertEqual(a.args, ["run", "--run-id", "x", "--force"])

    def test_atlh_script_uses_runner_config(self):
        r = rig.Runner(name="t", ssh="x", arch="aarch64", config="config.aarch64.toml")
        self.assertEqual(shlex.split(rig.atlh_script(r, ["rank", "a b"])),
                         ["cargo", "run", "--release", "--quiet", "--", "--config", "config.aarch64.toml",
                          "rank", "a b"])


class Bootstrap(unittest.TestCase):
    def test_unknown_step_rejected(self):
        with tempfile.TemporaryDirectory() as home:
            p = subprocess.run(["bash", str(REPO / "scripts/runner/bootstrap.sh")],
                               env={**os.environ, "HOME": home, "STEPS": "packages bogus"},
                               capture_output=True, text=True)
            self.assertEqual(p.returncode, 2)
            self.assertIn("unknown step 'bogus'", p.stderr)

    def test_steps_match(self):
        text = (REPO / "scripts/runner/bootstrap.sh").read_text()
        self.assertIn('ALL_STEPS="' + " ".join(rig.BOOTSTRAP_STEPS) + '"', text)


if __name__ == "__main__":
    unittest.main()


class BoundedWaits(unittest.TestCase):
    """wait_job always ends with a verdict (simulated clock, no ssh)."""

    def run_wait(self, states, **kw):
        clock = [0.0]
        seq = iter(states)
        orig = (rig.job_state, rig.time.time, rig.time.sleep, rig.run_remote)
        rig.job_state = lambda r, n: next(seq)
        rig.time.time = lambda: clock[0]
        rig.time.sleep = lambda s: clock.__setitem__(0, clock[0] + s)
        rig.run_remote = lambda *a, **k: None
        try:
            return rig.wait_job(rig.Runner(name="t", ssh="x", arch="x"), "j", interval=10, heartbeat=10**9, **kw)
        finally:
            rig.job_state, rig.time.time, rig.time.sleep, rig.run_remote = orig

    def test_exit_code_passed_through(self):
        self.assertEqual(self.run_wait([("running", None, 1), ("exit", "3", 5)]), 3)

    def test_stall(self):
        with self.assertRaises(rig.RigStalled):
            self.run_wait([("running", None, 7)] * 100, stall=60)

    def test_progress_resets_stall(self):
        states = [("running", None, i) for i in range(20)] + [("exit", "0", 99)]
        self.assertEqual(self.run_wait(states, stall=15), 0)

    def test_unreachable(self):
        with self.assertRaises(rig.RigUnreachable):
            self.run_wait([("unreachable", None, -1)] * 100, unreachable=60)

    def test_blip_is_survivable(self):
        states = [("running", None, 1), ("unreachable", None, -1), ("unreachable", None, -1),
                  ("running", None, 2), ("exit", "0", 3)]
        self.assertEqual(self.run_wait(states, unreachable=60), 0)

    def test_max_time(self):
        states = [("running", None, i) for i in range(100)]
        with self.assertRaises(rig.RigTimeout):
            self.run_wait(states, max_time=100)

    def test_exit_codes(self):
        self.assertEqual((rig.RigTimeout.code, rig.RigStalled.code, rig.RigUnreachable.code), (124, 125, 126))
