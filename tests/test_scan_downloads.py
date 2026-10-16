"""labyrinth-scan-downloads with stand-ins for heimdalld and notify-send. Run: python tests/test_scan_downloads.py"""
import os
import stat
import subprocess
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-scan-downloads")


def stub(d, name, body):
    p = os.path.join(d, name)
    with open(p, "w") as f:
        f.write("#!/bin/sh\n" + body + "\n")
    os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)
    return p


class Scan(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.home = os.path.join(self.d, "home")
        os.makedirs(os.path.join(self.home, "Downloads"))
        self.log = os.path.join(self.d, "calls.log")
        # heimdalld: records its arguments, reports one infected file and exits 1 like the real one
        self.heim = stub(self.d, "heimdalld", 'echo "heimdalld $*" >> %s\necho "INFECTED /home/u/Downloads/bad.exe: Test.Virus"\necho "scanned 2 files in 1s: 1 infected, 0 could not be scanned"\nexit 1' % self.log)
        stub(self.d, "notify-send", 'echo "notify $*" >> %s' % self.log)
        self.env = {"PATH": self.d + ":/usr/bin:/bin", "HOME": self.home, "HEIMDALLD": self.heim}

    def run_it(self, heim=None):
        env = dict(self.env)
        if heim:
            env["HEIMDALLD"] = heim
        return subprocess.run(["python3", SCRIPT], capture_output=True, text=True, env=env)

    def calls(self):
        return open(self.log).read().splitlines() if os.path.exists(self.log) else []

    def test_a_find_is_notified_once(self):
        r = self.run_it()
        self.assertEqual(r.returncode, 1)
        notes = [c for c in self.calls() if c.startswith("notify")]
        self.assertEqual(len(notes), 1)
        self.assertIn("Possible malware found", notes[0])
        self.assertIn("bad.exe", notes[0])
        self.run_it()  # the same file again: no second notification
        self.assertEqual(len([c for c in self.calls() if c.startswith("notify")]), 1)

    def test_arguments_carry_state_exclusions_and_folders(self):
        conf = os.path.join(self.home, ".config", "labyrinth")
        os.makedirs(conf)
        open(os.path.join(conf, "scan-exclude"), "w").write("# comment\n*.iso\n~/Downloads/big\n")
        extra = os.path.join(self.home, "Extra")
        os.makedirs(extra)
        open(os.path.join(conf, "scan-folders"), "w").write(extra + "\n")
        self.run_it()
        args = [c for c in self.calls() if c.startswith("heimdalld")][0]
        self.assertIn("scan-files --state", args)
        self.assertIn("--exclude *.iso", args)
        self.assertIn("--exclude " + os.path.join(self.home, "Downloads/big"), args)
        self.assertTrue(args.endswith(os.path.join(self.home, "Downloads") + " " + extra), args)

    def test_a_scanner_that_cannot_run_is_quiet(self):
        broken = stub(self.d, "heimdalld2", 'echo "heimdalld scan-files: ClamAV is not installed" >&2; exit 2')
        r = self.run_it(heim=broken)
        self.assertEqual(r.returncode, 0)
        self.assertFalse([c for c in self.calls() if c.startswith("notify")])
        log = open(os.path.join(self.home, ".local", "state", "labyrinth", "scan.log")).read()
        self.assertIn("scan did not run", log)

    def test_no_downloads_folder_is_not_an_error(self):
        os.rmdir(os.path.join(self.home, "Downloads"))
        self.assertEqual(self.run_it().returncode, 0)


if __name__ == "__main__":
    unittest.main()
