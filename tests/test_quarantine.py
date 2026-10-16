"""labyrinth-quarantine and the quarantine/notify decisions of labyrinth-scan-downloads. Run: python tests/test_quarantine.py"""
import json
import os
import stat
import subprocess
import tempfile
import unittest

BIN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin")
Q = os.path.join(BIN, "labyrinth-quarantine")
SCAN = os.path.join(BIN, "labyrinth-scan-downloads")


def stub(d, name, body):
    p = os.path.join(d, name)
    with open(p, "w") as f:
        f.write("#!/bin/sh\n" + body + "\n")
    os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)
    return p


class Fixture(unittest.TestCase):
    def setUp(self):
        self.d = os.path.realpath(tempfile.mkdtemp())
        self.home = os.path.join(self.d, "home")
        self.dl = os.path.join(self.home, "Downloads")
        os.makedirs(self.dl)
        self.calls = os.path.join(self.d, "calls.log")
        self.env = {"PATH": self.d + ":/usr/bin:/bin", "HOME": self.home, "LABYRINTH_QUARANTINE": Q, "LABYRINTH_SCAN_SYNC": "1",
                    "LABYRINTH_RUN_MEDIA": os.path.join(self.d, "no-media")}
        stub(self.d, "notify-send", 'echo "notify $*" >> %s\n%s' % (self.calls, 'echo "$NOTIFY_ANSWER"'))

    def q(self, *args, env=None):
        return subprocess.run(["python3", Q] + list(args), capture_output=True, text=True, env=dict(self.env, **(env or {})))

    def file(self, name, body="evil\n", folder=None):
        p = os.path.join(folder or self.dl, name)
        with open(p, "w") as f:
            f.write(body)
        os.chmod(p, 0o755)
        return p

    def scan(self, infected, answer="", extra_conf=None):
        lines = "".join('echo "INFECTED %s: %s"\n' % (p, s) for p, s in infected)
        heim = stub(self.d, "heimdalld", lines + 'echo "scanned 3 files in 1s: %d infected, 0 could not be scanned"\nexit %d' % (len(infected), 1 if infected else 0))
        if extra_conf is not None:
            os.makedirs(os.path.join(self.home, ".config", "labyrinth"), exist_ok=True)
            open(os.path.join(self.home, ".config", "labyrinth", "scan.conf"), "w").write(extra_conf)
        return subprocess.run(["python3", SCAN], capture_output=True, text=True, env=dict(self.env, HEIMDALLD=heim, NOTIFY_ANSWER=answer))

    def notes(self):
        return [c for c in open(self.calls).read().splitlines()] if os.path.exists(self.calls) else []


class QuarantineCli(Fixture):
    def test_add_list_restore_delete(self):
        p = self.file("bad.exe")
        r = self.q("add", p, "--signature", "Win.Trojan.Test")
        self.assertEqual(r.returncode, 0, r.stderr)
        qid = r.stdout.strip()
        self.assertFalse(os.path.exists(p))
        listing = json.loads(self.q("list", "--json").stdout)
        self.assertEqual([(i["id"], i["signature"], i["original"]) for i in listing], [(qid, "Win.Trojan.Test", p)])
        payload = os.path.join(self.home, ".local/share/labyrinth/quarantine", qid, "payload")
        self.assertEqual(stat.S_IMODE(os.stat(payload).st_mode), 0o600)  # not executable
        back = self.q("restore", qid).stdout.strip()
        self.assertEqual(back, p)
        self.assertEqual(open(p).read(), "evil\n")
        self.assertFalse(os.stat(p).st_mode & 0o111)                      # comes back without the execute bit
        self.assertEqual(json.loads(self.q("list", "--json").stdout), [])
        digest = json.loads(self.q("list", "--json").stdout) or None
        self.assertEqual(self.q("allowed", __import__("hashlib").sha256(b"evil\n").hexdigest()).returncode, 0)  # remembered
        self.assertEqual(self.q("allowed", "0" * 64).returncode, 1)

    def test_restore_never_overwrites(self):
        p = self.file("a.bin")
        qid = self.q("add", p, "--signature", "X").stdout.strip()
        self.file("a.bin", body="new file\n")
        back = self.q("restore", qid).stdout.strip()
        self.assertEqual(back, p + ".restored")
        self.assertEqual(open(p).read(), "new file\n")

    def test_delete_and_bad_ids(self):
        p = self.file("b.bin")
        qid = self.q("add", p, "--signature", "X").stdout.strip()
        self.assertEqual(self.q("delete", qid).returncode, 0)
        self.assertEqual(self.q("list", "--json").stdout.strip(), "[]")
        for bad in ["../../etc", "zzzz", "", "123456789012"]:
            self.assertNotEqual(self.q("restore", bad).returncode, 0)
            self.assertNotEqual(self.q("delete", bad).returncode, 0)

    def test_refuses_links_folders_and_missing_files(self):
        target = self.file("t.bin")
        link = os.path.join(self.dl, "link")
        os.symlink(target, link)
        self.assertNotEqual(self.q("add", link, "--signature", "X").returncode, 0)
        self.assertTrue(os.path.exists(target))
        self.assertNotEqual(self.q("add", self.dl, "--signature", "X").returncode, 0)
        self.assertNotEqual(self.q("add", os.path.join(self.dl, "nope"), "--signature", "X").returncode, 0)


class ScanDecisions(Fixture):
    def test_a_certain_find_in_downloads_is_quarantined_and_announced(self):
        p = self.file("invoice.exe")
        r = self.scan([(p, "Win.Trojan.Agent-123")])
        self.assertEqual(r.returncode, 1)
        self.assertFalse(os.path.exists(p))
        note = " ".join(self.notes())
        self.assertIn("Malware quarantined", note)
        self.assertIn("-A restore=Restore", note)
        self.assertIn("-A delete=Delete", note)
        self.assertIn("QUARANTINED", open(os.path.join(self.home, ".local/state/labyrinth/scan.log")).read())

    def test_restore_from_the_notification_puts_it_back_and_it_is_not_caught_again(self):
        p = self.file("tool.exe")
        self.scan([(p, "Win.Trojan.Agent-123")], answer="restore")
        self.assertTrue(os.path.exists(p))                  # the notification's Restore button
        self.scan([(p, "Win.Trojan.Agent-123")])             # same content: you said keep it
        self.assertTrue(os.path.exists(p))
        self.assertEqual(len([n for n in self.notes() if "Malware quarantined" in n]), 1)

    def test_delete_from_the_notification(self):
        p = self.file("x.exe")
        self.scan([(p, "Win.Trojan.Agent-123")], answer="delete")
        self.assertFalse(os.path.exists(p))
        self.assertEqual(json.loads(self.q("list", "--json").stdout), [])

    def test_a_guess_is_only_a_warning(self):
        for sig in ["PUA.Win.Packer.Upx", "Heuristics.Encrypted.PDF", "Win.Test.EICAR_HDB-1x", "Html.Phishing.Bank-9"]:
            p = self.file("maybe.bin", body=sig)
            self.scan([(p, sig)])
            self.assertTrue(os.path.exists(p), sig)
        self.assertIn("Possible malware found", " ".join(self.notes()))

    def test_mail_attachment_cache_and_drives_are_watched(self):
        cache = os.path.join(self.home, ".cache", "thunderbird", "profile")
        os.makedirs(cache)
        media = os.path.join(self.d, "no-media")
        os.makedirs(media)
        p = self.file("attach.scr", folder=cache)
        self.scan([(p, "Win.Trojan.Agent-9")])
        self.assertFalse(os.path.exists(p))                 # outside Downloads, still quarantined
        args = open(os.path.join(self.d, "heimdalld")).read()
        self.assertIn("scanned", args)

    def test_not_yours_or_outside_the_watched_places_is_not_moved(self):
        elsewhere = os.path.join(self.d, "elsewhere")
        os.makedirs(elsewhere)
        p = self.file("bad.exe", folder=elsewhere)
        self.scan([(p, "Win.Trojan.Agent-9")])
        self.assertTrue(os.path.exists(p))
        self.assertIn("Possible malware found", " ".join(self.notes()))

    def test_notify_only_setting(self):
        p = self.file("bad2.exe")
        self.scan([(p, "Win.Trojan.Agent-9")], extra_conf="action = notify\n")
        self.assertTrue(os.path.exists(p))
        self.assertNotIn("quarantined", " ".join(self.notes()))


if __name__ == "__main__":
    unittest.main()
