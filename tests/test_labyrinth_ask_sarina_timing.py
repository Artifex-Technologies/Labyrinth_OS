"""labyrinth-ask-sarina's timing line (LABYRINTH_VOICE_TIMING=1) with a stand-in for sarina: no model, no network."""
import os
import re
import stat
import subprocess
import sys
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-ask-sarina")
FAKE_SARINA = """#!/bin/sh
case "$1" in
  route) echo services ;;
  chat|resume) printf 'Nothing is slowing it down. The load is low.\\n' ;;
esac
"""


class Timing(unittest.TestCase):
    def run_it(self, timing, whisper=None):
        d = tempfile.mkdtemp()
        bin_ = os.path.join(d, "bin")
        os.makedirs(bin_)
        p = os.path.join(bin_, "sarina")
        with open(p, "w") as f:
            f.write(FAKE_SARINA)
        os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)
        env = {"PATH": bin_ + ":/usr/bin:/bin", "HOME": d, "XDG_CONFIG_HOME": os.path.join(d, ".config"), "XDG_RUNTIME_DIR": d,
               "SARINA_SESSION_DIR": os.path.join(d, "sessions"), "SARINA_VAULT": os.path.join(d, "vault"), "LABYRINTH_TALK_LOG": "0",
               "SARINA_EMBED_URL": "http://127.0.0.1:1/api/embed"}
        if timing:
            env["LABYRINTH_VOICE_TIMING"] = "1"
        if whisper:
            env["LABYRINTH_VOICE_WHISPER_S"] = whisper
        r = subprocess.run([sys.executable, "-I", SCRIPT, "--voice", "--cwd", d, "is my computer running slow right now"],
                           capture_output=True, text=True, env=env, timeout=60)
        log = os.path.join(d, "labyrinth-sarina", "timing.log")
        return r, (open(log).read() if os.path.exists(log) else None)

    def test_one_line_with_every_stage_in_order(self):
        r, log = self.run_it(True, whisper="1.25")
        self.assertIn("Nothing is slowing it down.", r.stdout)
        self.assertIsNotNone(log, r.stderr)
        lines = log.splitlines()
        self.assertEqual(len(lines), 1, log)
        stages = re.findall(r"(\w+)=(\d+\.\d\d)", lines[0])
        self.assertEqual([s for s, _ in stages], ["whisper", "route", "prefetch", "first_token", "first_sentence", "done"], lines[0])
        self.assertEqual(stages[0][1], "1.25")
        after = [float(v) for _, v in stages[1:]]
        self.assertEqual(after, sorted(after), lines[0])

    def test_nothing_written_when_off(self):
        r, log = self.run_it(False)
        self.assertIn("Nothing is slowing it down.", r.stdout)
        self.assertIsNone(log)


if __name__ == "__main__":
    unittest.main()
