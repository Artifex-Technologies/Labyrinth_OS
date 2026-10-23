"""labyrinth-sarina-ask with stand-ins for whisper-cli, curl, sarina and labyrinth-ask-sarina: no audio, no model, no network."""
import os
import stat
import subprocess
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-sarina-ask")


def stub(d, name, body):
    p = os.path.join(d, name)
    with open(p, "w") as f:
        f.write("#!/bin/sh\n" + body + "\n")
    os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)


class SarinaAsk(unittest.TestCase):
    def run_it(self, whisper=None, model=True, ask="echo '**Sure.** It is `ok`.'", ollama=True, home_conf=None, tone=None, env_extra=None):
        d = tempfile.mkdtemp()
        stub(d, "sarina", "exit 0")
        stub(d, "curl", "exit 0" if ollama else "exit 7")
        stub(d, "labyrinth-ask-sarina", ask)
        if tone is not None:
            stub(d, "sarina-tone", tone)
        if whisper is not None:
            stub(d, "whisper-cli", whisper)
        wav = os.path.join(d, "q.wav")
        open(wav, "wb").write(b"RIFF")
        if home_conf:
            os.makedirs(os.path.join(d, ".config", "labyrinth"))
            open(os.path.join(d, ".config", "labyrinth", "sarina.env"), "w").write(home_conf)
        env = {"PATH": d + ":/usr/bin:/bin", "HOME": d, "LABYRINTH_WHISPER": os.path.join(d, "whisper-cli")}
        if model:
            m = os.path.join(d, "ggml-test.bin")
            open(m, "wb").write(b"x")
            env["LABYRINTH_STT_MODEL"] = m
        env.update(env_extra or {})
        return subprocess.run(["bash", SCRIPT, wav], capture_output=True, text=True, env=env)

    def test_a_tone_hint_reaches_her_prompt_only_when_switched_on(self):
        asks = 'echo "$@" | tr -d "\\n"'
        r = self.run_it(whisper="echo what time is it", ask=asks, tone="echo The owner sounds rushed: be brief.")
        self.assertIn("Style hint, never mention it", r.stdout)
        self.assertIn("sounds rushed", r.stdout)
        r = self.run_it(whisper="echo what time is it", ask=asks, tone="echo The owner sounds rushed: be brief.", env_extra={"LABYRINTH_TONE": "0"})
        self.assertNotIn("sounds rushed", r.stdout)
        r = self.run_it(whisper="echo what time is it", ask=asks, tone="exit 0")  # no hint (nothing unusual, or not enough history)
        self.assertNotIn("Style hint", r.stdout)

    def test_laughter_is_passed_on_and_not_shown_as_words(self):
        asks = 'echo "$@"'
        r = self.run_it(whisper="echo '[laughs] okay that one was good'", ask=asks)
        self.assertIn("(They laughed.) okay that one was good", r.stdout)
        r = self.run_it(whisper="echo '*chuckles*'", ask=asks)  # only laughter is still something said
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("(They laughed.) Haha.", r.stdout)
        r = self.run_it(whisper="echo what time is it", ask=asks)
        self.assertNotIn("laughed", r.stdout)

    def test_no_recording(self):
        r = subprocess.run(["bash", SCRIPT, "/nonexistent.wav"], capture_output=True, text=True)
        self.assertEqual(r.returncode, 1)
        self.assertIn("no recording", r.stdout)

    def test_not_set_up_says_so_instead_of_failing_obscurely(self):
        r = self.run_it(whisper=None)
        self.assertEqual(r.returncode, 1)
        self.assertIn("not set up", r.stdout)
        r = self.run_it(whisper="echo hi", model=False)
        self.assertIn("not set up", r.stdout)

    def test_model_server_down(self):
        r = self.run_it(whisper="echo hi", ollama=False)
        self.assertEqual(r.returncode, 1)
        self.assertIn("model server", r.stdout)

    def test_silence(self):
        r = self.run_it(whisper="echo ' [BLANK_AUDIO] '")
        self.assertEqual(r.returncode, 1)
        self.assertIn("did not catch", r.stdout)

    def test_stock_phrase_on_silence_is_not_a_question(self):
        for heard in ("you", "Thank you.", "Thanks for watching!"):
            r = self.run_it(whisper="echo '%s'" % heard)
            self.assertEqual(r.returncode, 1, heard)
            self.assertIn("did not catch", r.stdout)

    def test_answer_shows_what_was_heard_and_is_plain(self):
        r = self.run_it(whisper="echo ' What time is it? '")
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn('Heard: "What time is it?"', r.stdout)
        self.assertIn("Sure. It is ok.", r.stdout)
        self.assertNotIn("**", r.stdout)

    def test_voice_model_is_used_when_set(self):
        d = tempfile.mkdtemp()
        os.makedirs(os.path.join(d, ".config", "labyrinth"))
        open(os.path.join(d, ".config", "labyrinth", "sarina.env"), "w").write("OPENAI_MODEL=big\nVOICE_MODEL=small\n")
        r = self.run_it(whisper="echo hello", ask='echo "model=$OPENAI_MODEL"')
        self.assertIn("model=", r.stdout)  # no VOICE_MODEL in that run's HOME: the usual model (unset here)
        # with the file in HOME
        import shutil
        shutil.rmtree(d)
        r = self.run_it(whisper="echo hello", ask='echo "model=$OPENAI_MODEL"', home_conf="OPENAI_MODEL=big\nVOICE_MODEL=small\n")
        self.assertIn("model=small", r.stdout)

    def test_heard_line_comes_first(self):
        r = self.run_it(whisper="echo hello there")
        self.assertTrue(r.stdout.startswith('Heard: "hello there"\n'), r.stdout)

    def test_think_block_removed(self):
        r = self.run_it(whisper="echo hello", ask="printf '<think>\\nhmm\\n</think>\\n\\nFine.\\n'")
        self.assertEqual(r.returncode, 0)
        self.assertNotIn("think", r.stdout)
        self.assertIn("Fine.", r.stdout)


if __name__ == "__main__":
    unittest.main()
