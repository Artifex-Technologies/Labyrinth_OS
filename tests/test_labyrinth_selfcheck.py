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

LOG = """[2026-10-14T10:00:00-0600] [ALPM] installed kwin (6.7.5-3.1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded kwin (6.7.5-3.1 -> 6.8.0-1.1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded plasma-workspace (6.7.5-3.1 -> 6.8.0-1.1)
[2026-10-16T07:00:00-0600] [ALPM] upgraded kwin-extra (1 -> 2)
"""


class SelfCheck(unittest.TestCase):
    def test_last_upgrade_picks_the_latest_and_only_that_package(self):
        with tempfile.NamedTemporaryFile("w", delete=False) as f:
            f.write(LOG)
        want = time.mktime(time.strptime("2026-10-16T06:32:12", "%Y-%m-%dT%H:%M:%S"))
        self.assertEqual(sc.last_upgrade_epoch("kwin", f.name), want)  # not kwin-extra, not the older install
        self.assertIsNone(sc.last_upgrade_epoch("nothing", f.name))
        self.assertIsNone(sc.last_upgrade_epoch("kwin", "/nonexistent"))
        os.unlink(f.name)

    def test_stale_session(self):
        self.assertTrue(sc.stale_session(1000, 5000))
        self.assertFalse(sc.stale_session(5000, 1000))  # started after the upgrade
        self.assertFalse(sc.stale_session(4990, 5000))  # within the slack
        self.assertFalse(sc.stale_session(None, 5000))
        self.assertFalse(sc.stale_session(1000, None))

    def test_output_in_use_with_and_without_enabled(self):
        self.assertTrue(sc.output_in_use({"enabled": True}))
        self.assertFalse(sc.output_in_use({"enabled": False, "connected": True, "currentModeId": "6"}))
        self.assertTrue(sc.output_in_use({"connected": True, "currentModeId": "6"}))  # Plasma 6.8
        self.assertFalse(sc.output_in_use({"connected": False}))


if __name__ == "__main__":
    unittest.main()
