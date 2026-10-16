"""Tests for labyrinth-launch. Run: python tests/test_labyrinth_launch.py"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_labyrinth_apps import ROOT, load_module  # noqa: E402

launch = load_module("labyrinth_launch", os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-launch"))


class LaunchTests(unittest.TestCase):
    def test_exec_field_codes(self):
        e = launch.expand_exec
        self.assertEqual(e("minos %U", ["/a b", "/c"]), ["minos", "/a b", "/c"])
        self.assertEqual(e("viewer %f", ["/x", "/y"]), ["viewer", "/x"])
        self.assertEqual(e("app %i %c --name=%c 100%%", name="My App", icon="ic"), ["app", "--icon", "ic", "My App", "--name=My App", "100%"])
        self.assertEqual(e('sh -c "echo hi %u"', []), ["sh", "-c", "echo hi "])
        self.assertEqual(e("app %F"), ["app"])

    def test_slice_names_have_no_dash_and_are_stable(self):
        s = launch.slice_name("com.mitchellh.ghostty")
        self.assertEqual(s, "lapp_com_mitchellh_ghostty.slice")
        self.assertNotIn("-", launch.slice_name("spotify-launcher"))
        self.assertEqual(launch.slice_name("a-b"), launch.slice_name("a-b"))

    def test_policy_defaults_and_usage_counting(self):
        with tempfile.TemporaryDirectory() as d:
            os.environ["XDG_CONFIG_HOME"] = os.path.join(d, "c")
            os.environ["XDG_DATA_HOME"] = os.path.join(d, "d")
            self.assertEqual(launch.load_policy("x"), {})
            os.makedirs(os.path.join(d, "c", "labyrinth", "apps"))
            with open(launch.policy_path("x"), "w") as f:
                json.dump({"sandbox": True, "files": "folders"}, f)
            self.assertEqual(launch.load_policy("x")["files"], "folders")
            launch.count_launch("x", now=100)
            launch.count_launch("x", now=200)
            with open(os.path.join(d, "d", "labyrinth", "app-usage.json")) as f:
                self.assertEqual(json.load(f), {"x": {"launches": 2, "last": 200}})

    def test_build_command_wraps_in_a_scope_and_optionally_a_sandbox(self):
        with tempfile.TemporaryDirectory() as d:
            os.environ["XDG_CONFIG_HOME"] = os.path.join(d, "c")
            os.environ["XDG_DATA_HOME"] = os.path.join(d, "d")
            os.environ["XDG_DATA_DIRS"] = os.path.join(d, "share")
            apps = os.path.join(d, "share", "applications")
            os.makedirs(apps)
            with open(os.path.join(apps, "demo.desktop"), "w") as f:
                f.write("[Desktop Entry]\nType=Application\nName=Demo\nExec=demo --open %U\nIcon=demo\nPath=/tmp\n")
            cmd, cwd = launch.build_command("demo", ["/file"])
            self.assertEqual(cmd[:4], ["systemd-run", "--user", "--scope", "--quiet"])
            self.assertIn("--slice=lapp_demo.slice", cmd)
            self.assertEqual(cmd[-3:], ["demo", "--open", "/file"])
            self.assertEqual(cwd, "/tmp")
            self.assertIsNone(launch.build_command("nothing-here"))
            os.makedirs(os.path.join(d, "c", "labyrinth", "apps"))
            with open(launch.policy_path("demo"), "w") as f:
                json.dump({"sandbox": True, "files": "folders", "see_others": False}, f)
            cmd, _ = launch.build_command("demo", [])
            if launch.shutil.which("bwrap"):
                self.assertIn("bwrap", cmd)
                self.assertIn("--unshare-pid", cmd)
                self.assertIn("--tmpfs", cmd)
                self.assertEqual(cmd[cmd.index("--") + 1:], ["demo", "--open"])  # after the sandbox's own arguments: the app's command


    def test_network_rule_is_read_for_this_user_only(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "r.json")
            me, other = str(os.getuid()), str(os.getuid() + 1)
            with open(p, "w") as f:
                json.dump({"rules": {me: {"a": "block", "b": "bogus", "c": "wifi"}, other: {"d": "block"}}}, f)
            self.assertEqual(launch.network_rule("a", p), "block")
            self.assertEqual(launch.network_rule("c", p), "wifi")
            self.assertIsNone(launch.network_rule("b", p))
            self.assertIsNone(launch.network_rule("d", p))
            self.assertIsNone(launch.network_rule("a", os.path.join(d, "none.json")))


if __name__ == "__main__":
    unittest.main()
