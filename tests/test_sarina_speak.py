"""labyrinth-sarina-speak with a stand-in for Piper and for the audio player (no sound is made). Run: python tests/test_sarina_speak.py"""
import os
import stat
import subprocess
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-sarina-speak")


def make(path, body):
    with open(path, "w") as f:
        f.write("#!/bin/sh\n" + body + "\n")
    os.chmod(path, os.stat(path).st_mode | stat.S_IEXEC)


class Speak(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.tts = os.path.join(self.d, "tts")
        os.makedirs(os.path.join(self.tts, "voices"))
        open(os.path.join(self.tts, "voices", "en_US-amy-medium.onnx"), "w").write("x")
        open(os.path.join(self.tts, "voices", "other-voice.onnx"), "w").write("x")
        self.said = os.path.join(self.d, "said.txt")
        self.played = os.path.join(self.d, "played.log")
        self.piper = os.path.join(self.d, "py")
        # "python -m piper -m MODEL -f WAV": record the text and the model, write a file
        make(self.piper, 'cat > %s; echo "$4" > %s.model; echo wav > "$6"' % (self.said, self.said))
        self.player = os.path.join(self.d, "player")
        make(self.player, 'echo "played $1" >> %s' % self.played)
        self.env = {"PATH": "/usr/bin:/bin", "HOME": self.d, "XDG_RUNTIME_DIR": self.d, "LABYRINTH_TTS_DIR": self.tts, "LABYRINTH_TTS_PYTHON": self.piper, "LABYRINTH_PLAYER": self.player, "LABYRINTH_NO_ENVELOPE": "1"}

    def run_it(self, *args, stdin="", env=None):
        return subprocess.run(["bash", SCRIPT, *args], input=stdin, capture_output=True, text=True, env=dict(self.env, **(env or {})))

    def test_it_cleans_the_text_and_plays_the_speech(self):
        r = self.run_it(stdin="<think>hidden thoughts</think>\n**It is** `noon`.\n\nSee [the docs](http://x.y) for more.\n")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(open(self.said).read(), "It is noon. See the docs for more.")
        self.assertIn("amy-medium.onnx", open(self.said + ".model").read())
        self.assertIn("played", open(self.played).read())

    def test_the_chosen_voice_is_used(self):
        os.makedirs(os.path.join(self.d, ".config", "labyrinth"))
        open(os.path.join(self.d, ".config", "labyrinth", "sarina-voice"), "w").write("other-voice\n")
        self.run_it("Hello")
        self.assertIn("other-voice.onnx", open(self.said + ".model").read())

    def test_bad_voice_names_and_missing_voices_are_refused(self):
        r = self.run_it("Hello", env={"LABYRINTH_SARINA_VOICE": "../../etc/passwd"})
        self.assertNotEqual(r.returncode, 0)
        r = self.run_it("Hello", env={"LABYRINTH_SARINA_VOICE": "no-such-voice"})
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("not set up", r.stdout)
        self.assertFalse(os.path.exists(self.played))

    def test_nothing_to_say_is_silence_and_long_text_is_cut(self):
        self.assertEqual(self.run_it(stdin="<think>only thoughts</think>").returncode, 0)
        self.assertFalse(os.path.exists(self.said))
        self.run_it(stdin="word " * 400)
        self.assertLessEqual(len(open(self.said).read()), 700)

    def test_it_can_write_a_file_instead_of_playing(self):
        out = os.path.join(self.d, "out.wav")
        self.assertEqual(self.run_it("--to", out, "Hello").returncode, 0)
        self.assertTrue(os.path.exists(out))
        self.assertFalse(os.path.exists(self.played))

    def test_stop_with_nothing_playing_is_fine(self):
        self.assertEqual(self.run_it("--stop").returncode, 0)


if __name__ == "__main__":
    unittest.main()
