"""labyrinth-ask-sarina --voice and the rules for a spoken answer: the caller's (LABYRINTH_VOICE_RULES) win; a caller that passes
none (the desktop's crash watch, through Sarina's integration file) gets the installed ones. A stand-in for sarina: no model."""
import os
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-ask-sarina")
INSTALLED = "/usr/share/labyrinth/sarina-voice-rules.txt"
FAKE_SARINA = """#!/bin/sh
case "$1" in
  route) echo services ;;
  chat|resume) for a in "$@"; do printf '%s\\n' "$a"; done > "$HOME/args.txt"; printf 'It crashed in its renderer.\\n' ;;
esac
"""


class VoiceRules(unittest.TestCase):
    def ask(self, rules=None):
        d = tempfile.mkdtemp()
        bin_ = os.path.join(d, "bin")
        os.makedirs(bin_)
        p = os.path.join(bin_, "sarina")
        with open(p, "w") as f:
            f.write(FAKE_SARINA)
        os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)
        env = {"PATH": bin_ + ":/usr/bin:/bin", "HOME": d, "XDG_CONFIG_HOME": os.path.join(d, ".config"), "XDG_RUNTIME_DIR": d,
               "SARINA_SESSION_DIR": os.path.join(d, "sessions"), "SARINA_VAULT": os.path.join(d, "vault"), "LABYRINTH_TALK_LOG": "0",
               "SARINA_EMBED_URL": "http://127.0.0.1:1/api/embed", "LABYRINTH_VOICE_STREAM": "0"}
        if rules is not None:
            env["LABYRINTH_VOICE_RULES"] = rules
        r = subprocess.run([sys.executable, "-I", SCRIPT, "--voice", "--cwd", d, "The program game just crashed (SIGSEGV). Look into it."],
                           capture_output=True, text=True, env=env, timeout=60)
        with open(os.path.join(d, "args.txt")) as f:
            return r, f.read()

    def test_the_callers_rules_win(self):
        r, args = self.ask("Rules the caller passed.")
        self.assertIn("It crashed in its renderer.", r.stdout)
        self.assertIn("Rules the caller passed.", args)

    @unittest.skipUnless(os.path.isfile(INSTALLED), "the voice rules are not installed here")
    def test_installed_rules_when_none_are_passed(self):
        with open(INSTALLED, encoding="utf-8") as f:
            first = f.read().strip().splitlines()[0][:60]
        r, args = self.ask()
        self.assertIn("It crashed in its renderer.", r.stdout)
        self.assertIn(first, args)

    def test_empty_rules_stay_empty(self):
        # labyrinth-sarina-ask exports what it read, even nothing: that is the caller's choice, not "none passed"
        if not os.path.isfile(INSTALLED):
            self.skipTest("the voice rules are not installed here")
        with open(INSTALLED, encoding="utf-8") as f:
            first = f.read().strip().splitlines()[0][:60]
        _, args = self.ask("")
        self.assertNotIn(first, args)


if __name__ == "__main__":
    unittest.main()
