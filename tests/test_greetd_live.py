"""The live medium's greetd config is the installed one plus the live user's autologin. python tests/test_greetd_live.py"""
import os
import tomllib
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ETC = os.path.join(ROOT, "profile", "airootfs", "etc")


def load(name):
    with open(os.path.join(ETC, "greetd", name), "rb") as f:
        return tomllib.load(f)


class GreetdLive(unittest.TestCase):
    def test_the_login_screen_is_the_same(self):
        live, installed = load("live.toml"), load("config.toml")
        self.assertEqual(live["terminal"], installed["terminal"])
        self.assertEqual(live["default_session"], installed["default_session"])

    def test_only_the_live_config_signs_in_by_itself(self):
        self.assertNotIn("initial_session", load("config.toml"))
        s = load("live.toml")["initial_session"]
        self.assertEqual(s["user"], "live")
        self.assertIn("yggdrasil-session", s["command"])

    def test_greetd_reads_the_live_config_and_the_installer_removes_it(self):
        dropin = open(os.path.join(ETC, "systemd", "system", "greetd.service.d", "live.conf")).read()
        self.assertIn("--config /etc/greetd/live.toml", dropin)
        cleanup = open(os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-install-cleanup")).read()
        self.assertIn("/etc/greetd/live.toml", cleanup)
        self.assertIn("/etc/systemd/system/greetd.service.d/live.conf", cleanup)


if __name__ == "__main__":
    unittest.main()
