"""Tests for labyrinth-selfcheck's pure parts. Run: python tests/test_labyrinth_selfcheck.py"""
import importlib.machinery
import importlib.util
import os
import tempfile
import time
import unittest

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
path = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-selfcheck")
loader = importlib.machinery.SourceFileLoader("labyrinth_selfcheck", path)
spec = importlib.util.spec_from_loader("labyrinth_selfcheck", loader)
sc = importlib.util.module_from_spec(spec)
loader.exec_module(sc)

LOG = """[2026-10-14T10:00:00-0600] [ALPM] installed hyprland (0.56.1-1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland (0.56.1-1 -> 0.56.2-1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded yggdrasil-lock (0.1.0-1 -> 0.1.0-2)
[2026-10-16T07:00:00-0600] [ALPM] upgraded hyprland-extra (1 -> 2)
"""


class SelfCheck(unittest.TestCase):
    def test_last_upgrade_picks_the_latest_and_only_that_package(self):
        with tempfile.NamedTemporaryFile("w", delete=False) as f:
            f.write(LOG)
        want = time.mktime(time.strptime("2026-10-16T06:32:12", "%Y-%m-%dT%H:%M:%S"))
        self.assertEqual(sc.last_upgrade_epoch("hyprland", f.name), want)  # not hyprland-extra, not the older install
        self.assertIsNone(sc.last_upgrade_epoch("nothing", f.name))
        self.assertIsNone(sc.last_upgrade_epoch("hyprland", "/nonexistent"))
        os.unlink(f.name)

    def test_stale_session(self):
        self.assertTrue(sc.stale_session(1000, 5000))
        self.assertFalse(sc.stale_session(5000, 1000))  # started after the upgrade
        self.assertFalse(sc.stale_session(4990, 5000))  # within the slack
        self.assertFalse(sc.stale_session(None, 5000))
        self.assertFalse(sc.stale_session(1000, None))

    def test_login_screen(self):
        conf = 'vt = 7\n[default_session]\ncommand = "cage -s -- /usr/bin/yggdrasil-greeter"\nuser = "greeter"\n'
        self.assertIsNone(sc.login_screen_problem("/usr/lib/systemd/system/greetd.service", conf))
        self.assertIn("enable", sc.login_screen_problem("", conf))
        self.assertIn("sddm.service", sc.login_screen_problem("/usr/lib/systemd/system/sddm.service", conf))
        self.assertIn("no greeter", sc.login_screen_problem("/usr/lib/systemd/system/greetd.service", "vt = 7\n"))

if __name__ == "__main__":
    unittest.main()
