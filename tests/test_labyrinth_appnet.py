"""Tests for labyrinth-appnet. Run: python tests/test_labyrinth_appnet.py"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_labyrinth_apps import ROOT, load_module  # noqa: E402

BIN = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin")
appnet = load_module("labyrinth_appnet", os.path.join(BIN, "labyrinth-appnet"))
launch = load_module("labyrinth_launch2", os.path.join(BIN, "labyrinth-launch"))


class AppnetTests(unittest.TestCase):
    def test_launcher_and_helper_agree_on_the_slice(self):
        for app in ["firefox", "com.mitchellh.ghostty", "spotify-launcher", "a b", "x_y"]:
            self.assertEqual(appnet.slice_name(app), launch.slice_name(app))

    def test_cgroup_path_and_level(self):
        self.assertEqual(appnet.cgroup_path(1000, "firefox"), "user.slice/user-1000.slice/user@1000.service/lapp_firefox.slice")
        self.assertEqual(appnet.rule_text(1000, "firefox", "block")[1].count('level 4 "'), 1)

    def test_modes(self):
        self.assertIsNone(appnet.rule_text(1000, "a", "allow"))
        block = appnet.rule_text(1000, "a", "block")
        self.assertTrue(any("127.0.0.53" in r for r in block) and any('oifname != "lo" drop' in r for r in block))
        eth = appnet.rule_text(1000, "a", "ethernet")[0]
        self.assertTrue('oifname != "lo"' in eth and 'oifname != "en*"' in eth and 'oifname != "eth*"' in eth and "wl*" not in eth)
        wifi = appnet.rule_text(1000, "a", "wifi")[0]
        self.assertTrue('oifname != "wl*"' in wifi and "en*" not in wifi)

    def test_ruleset_skips_apps_whose_slice_is_not_running_and_orders_users(self):
        rules = {"1000": {"b": "block", "a": "wifi"}, "1001": {"c": "block"}}
        up = {appnet.cgroup_path(1000, "a"), appnet.cgroup_path(1001, "c")}
        text = appnet.ruleset(rules, exists=lambda p: p in up)
        self.assertIn("lapp_a.slice", text)
        self.assertIn("lapp_c.slice", text)
        self.assertNotIn("lapp_b.slice", text)
        self.assertTrue(text.startswith("table inet labyrinth_apps\ndelete table inet labyrinth_apps\n"))

    def test_state_is_validated(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "s.json")
            with open(p, "w") as f:
                json.dump({"rules": {"1000": {"ok": "block", "../x": "block", "bad": "explode", "free": "allow"}, "x": 5}}, f)
            self.assertEqual(appnet.load_state(p), {"1000": {"ok": "block"}})
            appnet.save_state({"1000": {"a": "wifi"}}, p)
            self.assertEqual(appnet.load_state(p), {"1000": {"a": "wifi"}})
            self.assertEqual(os.stat(p).st_mode & 0o777, 0o644)
            self.assertEqual(appnet.load_state(os.path.join(d, "missing.json")), {})

    def test_app_ids_cannot_inject_into_the_ruleset(self):
        for bad in ['a"; drop', "a b", "", ".hidden", "x" * 200, "a/b"]:
            self.assertFalse(appnet.valid_app(bad), bad)
        self.assertTrue(appnet.valid_app("com.mitchellh.ghostty") and appnet.valid_app("spotify-launcher"))


if __name__ == "__main__":
    unittest.main()
