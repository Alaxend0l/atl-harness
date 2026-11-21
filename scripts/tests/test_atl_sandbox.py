# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests for scripts/runner/atl-sandbox: policies and the bwrap arguments they
give (no bwrap, ATL or display needed)."""
import importlib.machinery
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parents[2]
_loader = importlib.machinery.SourceFileLoader("atl_sandbox", str(REPO / "scripts" / "runner" / "atl-sandbox"))
_spec = importlib.util.spec_from_loader("atl_sandbox", _loader)
sb = importlib.util.module_from_spec(_spec)
_loader.exec_module(sb)

P = "android.permission."


def pairs(args, flag):
    """[(src, dst)] for every occurrence of a two-argument bwrap flag."""
    return [(args[i + 1], args[i + 2]) for i, a in enumerate(args) if a == flag]


class Policy(unittest.TestCase):
    def test_no_permissions_no_access(self):
        p = sb.derive_policy([])
        self.assertFalse(p["network"])
        self.assertEqual(p["media"], {})
        self.assertFalse(p["camera"])

    def test_permissions_map(self):
        p = sb.derive_policy([P + "INTERNET", P + "READ_MEDIA_AUDIO", P + "CAMERA"])
        self.assertTrue(p["network"])
        self.assertEqual(p["media"], {"music": "ro"})
        self.assertTrue(p["camera"])

    def test_write_storage_wins(self):
        p = sb.derive_policy([P + "READ_MEDIA_IMAGES", P + "WRITE_EXTERNAL_STORAGE"])
        self.assertEqual(set(p["media"].values()), {"rw"})
        self.assertEqual(set(p["media"]), {"music", "pictures", "videos", "download"})


class Args(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name) / "home"
        for d in ("Music", "Pictures", ".ssh"):
            (self.home / d).mkdir(parents=True)
        self.apk = self.home / "app.apk"
        self.apk.write_bytes(b"")
        self.env = mock.patch.dict(os.environ, {"HOME": str(self.home), "XDG_RUNTIME_DIR": str(self.home / "rt"),
                                                "XDG_DATA_HOME": str(self.home / ".local/share"),
                                                "DISPLAY": ":5"}, clear=True)
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def build(self, policy, **kw):
        kw.setdefault("rootfs", None)
        kw.setdefault("x11", True)
        kw.setdefault("dbus_socket", None)
        return sb.build_args(str(self.apk), ["-w", "1080"], policy, **kw)

    def test_isolation_basics(self):
        args, env = self.build(sb.derive_policy([]))
        for flag in ("--unshare-pid", "--unshare-net", "--new-session", "--die-with-parent", "--clearenv"):
            self.assertIn(flag, args)
        self.assertNotIn("--unshare-ipc", args)  # X11 needs MIT-SHM
        self.assertEqual(args[args.index("--cap-drop") + 1], "ALL")
        self.assertIn(str(self.home), [d for d in args[args.index("--tmpfs"):]])  # home is a tmpfs
        bound = [s for s, _ in pairs(args, "--bind") + pairs(args, "--ro-bind")]
        self.assertNotIn(str(self.home / ".ssh"), bound)
        self.assertFalse(any(s.startswith(str(self.home / "Music")) for s in bound))
        self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "disabled:")
        self.assertEqual(env["ATL_MEDIA_DIRS"], "")

    def test_app_data_is_the_only_writable_home_dir(self):
        args, _ = self.build(sb.derive_policy([]))
        rw = [s for s, _ in pairs(args, "--bind")]
        self.assertEqual(rw, [str(self.home / ".local/share/android_translation_layer/app.apk_")])

    def test_network_shared_only_with_internet(self):
        args, _ = self.build(sb.derive_policy([P + "INTERNET"]))
        self.assertNotIn("--unshare-net", args)

    def test_media_grant_ro(self):
        args, env = self.build(sb.derive_policy([P + "READ_MEDIA_AUDIO"]))
        self.assertIn((str(self.home / "Music"), str(self.home / "Music")), pairs(args, "--ro-bind"))
        self.assertNotIn(str(self.home / "Pictures"), [s for s, _ in pairs(args, "--ro-bind") + pairs(args, "--bind")])
        self.assertEqual(env["ATL_MEDIA_DIRS"], str(self.home / "Music"))

    def test_explicit_media_dirs_resolved(self):
        os.environ["ATL_MEDIA_DIRS"] = str(self.home / "Music")
        args, env = self.build(sb.derive_policy([P + "READ_EXTERNAL_STORAGE"]))
        self.assertIn((str(self.home / "Music"), str(self.home / "Music")), pairs(args, "--ro-bind"))

    def test_rootfs_is_read_only(self):
        args, _ = self.build(sb.derive_policy([]), rootfs="/r")
        self.assertIn(("/r", "/"), pairs(args, "--ro-bind"))
        self.assertNotIn("/r", [s for s, _ in pairs(args, "--bind")])

    def test_wayland_only_socket(self):
        os.environ["WAYLAND_DISPLAY"] = "wayland-1"
        args, env = self.build(sb.derive_policy([]), x11=False)
        self.assertEqual(env["GDK_BACKEND"], "wayland")
        self.assertIn("--unshare-ipc", args)
        self.assertFalse(any("X11-unix" in a for a in args))

    def test_dbus_goes_through_the_proxy_socket(self):
        args, env = self.build(sb.derive_policy([]), dbus_socket="/rt/proxy")
        self.assertIn(("/rt/proxy", str(self.home / "rt") + "/bus"), pairs(args, "--ro-bind"))
        self.assertTrue(env["DBUS_SESSION_BUS_ADDRESS"].endswith("/bus"))


if __name__ == "__main__":
    unittest.main()
