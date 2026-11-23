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
        self.assertEqual(p["network"], "none")
        self.assertEqual(p["media"], {})
        self.assertFalse(p["camera"])

    def test_permissions_map(self):
        p = sb.derive_policy([P + "INTERNET", P + "READ_MEDIA_AUDIO", P + "CAMERA"])
        self.assertEqual(p["network"], "internet")  # not the LAN: that needs an explicit grant
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
        kw.setdefault("dns_file", None)
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
        self.assertNotIn("/dev/snd", args)  # hw: bypasses the phone's speaker protection
        self.assertEqual(env["ATL_MEDIA_DIRS"], "")

    def test_app_data_is_the_only_writable_home_dir(self):
        args, _ = self.build(sb.derive_policy([]))
        rw = [s for s, _ in pairs(args, "--bind")]
        self.assertEqual(rw, [str(self.home / ".local/share/android_translation_layer/app.apk_")])

    def test_network_modes(self):
        full, _ = self.build({"network": "full"})
        self.assertNotIn("--unshare-net", full)
        self.assertNotIn("--unshare-user", full)
        inet, _ = self.build(sb.derive_policy([P + "INTERNET"]), dns_file="/x/resolv.conf")
        self.assertNotIn("--unshare-net", inet)  # the netns comes from launch_filtered()
        self.assertIn("--unshare-user", inet)
        self.assertIn(("/x/resolv.conf", sb.resolv_target(None)), pairs(inet, "--ro-bind"))
        self.assertEqual(sb.network_mode({"network": True}), "internet")  # old policies
        self.assertEqual(sb.network_mode({"network": False}), "none")
        self.assertEqual(sb.network_mode({"network": ["192.168.1.5"]}), ["192.168.1.5"])
        with self.assertRaises(SystemExit):
            sb.network_mode({"network": "lan"})


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

    def test_group_masked_android_style(self):
        # no rootfs: the host's group database is masked away
        args, _ = self.build(sb.derive_policy([]))
        ro = pairs(args, "--ro-bind")
        self.assertIn(("/dev/null", "/etc/group"), ro)
        self.assertNotIn(("/etc/group", "/etc/group"), ro)
        self.assertNotIn(("/dev/null", "/etc/passwd"), ro)  # art_standalone needs it
        # with a rootfs that has it: masked too, and not bound from the host
        root = Path(self.tmp.name) / "root"
        (root / "etc").mkdir(parents=True)
        for f in ("passwd", "group"):
            (root / "etc" / f).write_text("x:x:0:\n")
        args, _ = self.build(sb.derive_policy([]), rootfs=str(root))
        ro = pairs(args, "--ro-bind")
        self.assertIn(("/dev/null", "/etc/group"), ro)
        self.assertNotIn(("/etc/group", "/etc/group"), ro)

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


class Rules(unittest.TestCase):
    def test_internet_blocks_private_ranges_after_dns(self):
        r = sb.nft_rules([])
        self.assertLess(r.index("udp dport 53 accept"), r.index("192.168.0.0/16"))
        for net in ("127.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "169.254.0.0/16", "fc00::/7", "fe80::/10"):
            self.assertIn(net, r)
        self.assertIn("policy accept", r)

    def test_allow_list_before_the_block(self):
        r = sb.nft_rules(["192.168.10.5:8096", "100.64.0.0/10", "[fd7a:115c:a1e0::/48]:443"])
        self.assertIn("ip daddr 192.168.10.5 tcp dport 8096 accept", r)
        self.assertIn("ip daddr 192.168.10.5 udp dport 8096 accept", r)
        self.assertIn("ip daddr 100.64.0.0/10 accept", r)
        self.assertIn("ip6 daddr fd7a:115c:a1e0::/48 tcp dport 443 accept", r)
        self.assertLess(r.index("100.64.0.0/10 accept"), r.index("reject"))

    def test_bad_entries_rejected(self):
        for bad in ("192.168.1.999", "10.0.0.0/8:0", "host.local", "1.2.3.4:99999"):
            with self.subTest(bad), self.assertRaises(ValueError):
                sb.parse_allow(bad)

if __name__ == "__main__":
    unittest.main()
