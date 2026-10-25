"""labyrinth-sarina-checkup's security review: Heimdall's findings turned into keys, and Sarina's proposal saved with one notification
(sarina and notify-send are stand-ins). Run: python tests/test_sarina_checkup_security.py"""
import importlib.machinery
import importlib.util
import os
import stat
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load():
    path = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-sarina-checkup")
    loader = importlib.machinery.SourceFileLoader("checkup", path)
    mod = importlib.util.module_from_spec(importlib.util.spec_from_loader("checkup", loader))
    loader.exec_module(mod)
    return mod


AUDIT = {
    "packages": [{"package": "lua51", "installed": "5.1.5-13.1", "severity": "Low", "advisory": "AVG-1302"}],
    "kernels": [{"kernel": "7.3.0-4-cachyos", "running": True, "count": 0}, {"kernel": "6.18.55-2-cachyos-lts", "running": False, "count": 733}],
    "hardening": [{"id": "kvm-world-writable", "now": "/dev/kvm is 0666 (every account)"}],
}


class Security(unittest.TestCase):
    def test_keys(self):
        keys = load().security_keys(AUDIT)
        self.assertEqual(sorted(keys), ["hard|kvm-world-writable|/dev/kvm is 0666 (every account)", "kernel|6.18.55-2-cachyos-lts|733", "pkg|lua51|5.1.5-13.1|AVG-1302"])
        self.assertEqual(keys["kernel|6.18.55-2-cachyos-lts|733"], "kernel 6.18.55-2-cachyos-lts: 733 known holes")

    def test_review_is_saved_in_the_vault_and_notified(self):
        bin_dir, vault = tempfile.mkdtemp(), tempfile.mkdtemp()
        log = os.path.join(bin_dir, "log")
        for name, body in (("sarina", 'echo "$@" >> %s; echo "Summary: nothing urgent."; echo "Do now: 1. fix kvm"' % log),
                           ("notify-send", 'echo "notify $@" >> %s' % log)):
            p = os.path.join(bin_dir, name)
            with open(p, "w") as f:
                f.write("#!/bin/sh\n" + body + "\n")
            os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)
        mod = load()
        os.environ["PATH"] = bin_dir + os.pathsep + os.environ["PATH"]
        out = mod.security_review(dict(os.environ), vault, ["/dev/kvm is 0666 (every account)"])
        self.assertIn("saved in", out)
        sec = os.path.join(vault, "Security")
        text = open(os.path.join(sec, os.listdir(sec)[0])).read()
        self.assertIn("tags: [security]", text)
        self.assertIn("Do now: 1. fix kvm", text)
        calls = open(log).read()
        self.assertIn("skill run security-advisor new since the last check-up: /dev/kvm", calls)
        self.assertIn("notify -a Sarina -i security-high Security review nothing urgent.", calls)


class Held(unittest.TestCase):
    def test_a_broken_program_whose_file_is_held_asks_heimdall(self):
        mod = load()
        spool = tempfile.mkdtemp()
        mod.REQUESTS = spool
        held = [{"sha256": "ab" * 32, "original": "/home/u/.local/bin/mytool", "status": "held"},
                {"sha256": "cd" * 32, "original": "/home/u/.sarina/skills/evil.md", "status": "held"}]
        broken = [("{ path=/home/u/.local/bin/mytool ; argv[]=/home/u/.local/bin/mytool --serve ; }", "the service mytool.service failed (your session)"),
                  ("/usr/bin/spotify", "spotify crashed")]
        asked = mod.held_check(held, broken)
        self.assertEqual(asked, ["/home/u/.local/bin/mytool: the service mytool.service failed (your session)"])
        reqs = os.listdir(spool)
        self.assertEqual(len(reqs), 1)
        body = open(os.path.join(spool, reqs[0])).read()
        self.assertIn('"release_if_proven": "' + "ab" * 32, body)

    def test_nothing_held_nothing_asked(self):
        self.assertEqual(load().held_check([], [("x", "y")]), [])


if __name__ == "__main__":
    unittest.main()
