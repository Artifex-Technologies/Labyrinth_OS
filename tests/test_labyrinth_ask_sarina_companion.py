"""labyrinth-ask-sarina's companion side with a stand-in for sarina: what she is told when the talk is not about the computer, what is kept in her
vault (talks, laughs), and what is never kept. No model."""
import datetime
import importlib.machinery
import importlib.util
import os
import stat
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-ask-sarina")


def load():
    loader = importlib.machinery.SourceFileLoader("ask_sarina", SCRIPT)
    spec = importlib.util.spec_from_loader("ask_sarina", loader)
    m = importlib.util.module_from_spec(spec)
    loader.exec_module(m)
    return m


class Companion(unittest.TestCase):
    def setUp(self):
        self.m = load()
        self.d = tempfile.mkdtemp()
        self.vault = os.path.join(self.d, "vault")
        os.makedirs(os.path.join(self.vault, "Entities"))
        os.makedirs(os.path.join(self.vault, "Jokes"))
        with open(os.path.join(self.vault, "Entities", "Inside jokes.md"), "w") as f:
            f.write("# Inside jokes\n\n- 2026-10-17: Zlothra is Sloth plus Mothra.\n")
        with open(os.path.join(self.vault, "Jokes", "2026-10.md"), "w") as f:
            f.write("# Jokes\n\n- 2026-10-22 09:09 | they said: a printer joke | I said: It had a paper cut.\n")
        bin_ = os.path.join(self.d, "bin")
        os.makedirs(bin_)
        for name, body in (("sarina", "printf 'Talks/2026-10.md  [meaning]\\n  - 2026-10-20 21:00 | they said: my cat knocked the mug over | I said: Gravity tests.\\nEntities/Inside jokes.md  [meaning]\\n  - 2026-10-17: Zlothra is Sloth plus Mothra.\\nHome.md  [keywords]\\n  - 2026-10-01 not a moment\\n'"),
                           ("sarina-tone", "echo 'This week they have sounded low or tired in 5 of 12 questions.'")):
            p = os.path.join(bin_, name)
            with open(p, "w") as f:
                f.write("#!/bin/sh\n" + body + "\n")
            os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)
        self.path = os.environ["PATH"]
        os.environ["PATH"] = bin_ + ":" + self.path
        self.env = {"SARINA_VAULT": self.vault, "PATH": os.environ["PATH"]}

    def tearDown(self):
        os.environ["PATH"] = self.path

    def test_small_talk_gets_what_a_friend_would_know(self):
        c = self.m.companion("(They laughed.) the cat did it once more, tell me a joke about cats", self.env, datetime.datetime(2026, 10, 22, 22, 40))
        self.assertIn("Thursday late evening, 22:40", c)
        self.assertIn("low or tired in 5 of 12", c)
        self.assertIn("Sloth plus Mothra", c)
        c0 = self.m.companion("the cat did it once more", self.env)
        self.assertNotIn("Your inside jokes", c0)  # not asked about: only the moments that match by meaning
        self.assertIn("Sloth plus Mothra", c0)
        c2 = self.m.companion("why is my name funny again", dict(self.env, LABYRINTH_TONE="0"))
        self.assertIn("Your inside jokes", c2)
        self.assertEqual(c2.count("Sloth plus Mothra"), 1)
        self.assertNotIn("paper cut", c)  # what she told before is not shown to her (a small model retells it); a repeat is caught after
        self.assertIn("my cat knocked the mug over", c)
        self.assertTrue(c.endswith("What they said: "))
        c = self.m.companion("hello", dict(self.env, LABYRINTH_TONE="0"))
        self.assertNotIn("sounded", c)

    def test_notes_before_the_words_are_not_what_was_said(self):
        q = "(They laughed.) (Style hint, never mention it or say how they sound: be brief.) okay you win"
        self.assertEqual(self.m.spoken(q), "okay you win")
        self.assertTrue(self.m.LAUGH_RX.search(q))
        self.assertTrue(self.m.LAUGH_RX.search("hahaha okay"))
        self.assertFalse(self.m.LAUGH_RX.search("what is the load on the CPU"))

    def test_a_help_desk_offer_at_the_end_of_a_chat_answer_is_dropped(self):
        d = self.m.drop_offers
        self.assertEqual(d("That sounds tough. Would you like to talk about it, or is there anything I can do to help?"), "That sounds tough.")
        self.assertEqual(d("I'd go with yes. Let me know if you need anything else."), "I'd go with yes.")
        self.assertEqual(d("Drawing is a good pick. Want help finding tutorials or supplies?"), "Drawing is a good pick.")
        self.assertEqual(d("Drawing is a great start. (Though I'd hate to see your art.)"), "Drawing is a great start.")
        self.assertEqual(d("I'm sorry to hear that. Would you like to talk about what happened, or need help with something else? I'm here to listen."), "I'm sorry to hear that. I'm here to listen.")
        self.assertEqual(d("Would you like to talk about it?"), "Would you like to talk about it?")
        self.assertEqual(d("Want me to tell another?"), "Want me to tell another?")  # nothing else to say: kept
        self.assertEqual(d("Pixel ran gravity tests. Did the mug survive?"), "Pixel ran gravity tests. Did the mug survive?")

    def test_the_same_question_asked_before_is_not_recalled(self):
        self.assertEqual(self.m.recall("my cat knocked the mug over", self.vault, self.env), ["2026-10-17: Zlothra is Sloth plus Mothra."])
        self.assertEqual(len(self.m.recall("what about my cat", self.vault, self.env)), 2)

    def test_what_she_learned_is_known_but_forgotten_is_not(self):
        with open(os.path.join(self.vault, "Entities", "About Zlothra.md"), "w") as f:
            f.write("# About Zlothra\n\n## Observations\n\n"
                    "- Zlothra has a cat named Pixel — seen 3×, last 2026-10-01. Evidence: \"my cat Pixel\"\n"
                    "- Zlothra is learning to draw — seen 1×, last 2026-10-20. Evidence: \"picking up drawing\"\n"
                    "- Zlothra once mentioned rain — seen 1×, last 2026-09-01.\n"
                    "\n## Forgotten\n\n- Zlothra's sister is called Ana — seen 4×, last 2026-10-21.\n")
        got = self.m.learned(self.vault, datetime.date(2026, 10, 23))
        self.assertEqual(got, ["Zlothra has a cat named Pixel", "Zlothra is learning to draw"])
        env = dict(self.env, LABYRINTH_TONE="0", SARINA_EMBED_URL="http://127.0.0.1:9/none")  # no embeddings: shared words decide
        c = self.m.companion("hello there friend", env, datetime.datetime(2026, 10, 23, 12, 0))
        self.assertNotIn("What you have learned", c)  # nothing in it is about this
        c = self.m.companion("how is my cat doing, do you think", env, datetime.datetime(2026, 10, 23, 12, 0))
        self.assertIn("What you have learned about them that bears on this", c)
        self.assertIn("cat named Pixel", c)
        self.assertNotIn("Ana", c)
        self.assertNotIn("rain", c)

    def test_a_retold_joke_is_recognised(self):
        told = self.m.told_jokes(self.vault)
        self.assertEqual(told, ["It had a paper cut."])
        told = ["Why did the printer go to the doctor? It had a paper cut.", "Why don't backups get lonely? They have a backup plan."]
        self.assertTrue(self.m.a_repeat("[joking] Why did the printer go to the doctor? It had a paper cut!", told).endswith("paper cut."))
        self.assertTrue(self.m.a_repeat("Why did the printer see a doctor? Because it had a paper cut.", told))
        self.assertEqual(self.m.a_repeat("Why did the printer get promoted? It was outstanding in its field.", told), "")
        self.assertEqual(self.m.a_repeat("Ha.", told), "")

    def test_only_talk_that_is_not_about_the_computer_is_chat(self):
        self.assertFalse(self.m.about_the_computer([]))
        self.assertFalse(self.m.about_the_computer(["chat"]))
        self.assertTrue(self.m.about_the_computer(["performance"]))
        self.assertTrue(self.m.about_the_computer(["guides"]))
        self.assertTrue(self.m.about_the_computer([], "Update all my packages right now with sudo pacman -Syu."))
        self.assertFalse(self.m.about_the_computer([], "my sister got a new puppy"))

    def test_talk_is_kept_but_not_secrets_or_one_word_replies(self):
        self.m.log_talk("(Style hint, x: y.) my sister is visiting this weekend", "[gentle] That sounds nice. Are you cooking?", self.env)
        self.m.log_talk("thanks", "Any time.", self.env)
        self.m.log_talk("my wifi password is hunter2 by the way", "Noted.", self.env)
        with open(os.path.join(self.vault, "Talks", datetime.date.today().strftime("%Y-%m") + ".md")) as f:
            t = f.read()
        self.assertIn("they said: my sister is visiting this weekend | I said: That sounds nice. Are you cooking?", t)
        self.assertNotIn("Style hint", t)
        self.assertNotIn("Any time", t)
        self.assertNotIn("hunter2", t)
        off = dict(self.env, LABYRINTH_TALK_LOG="0")
        self.m.log_talk("my sister is back home now", "Good.", off)
        self.m.log_joke("tell me a joke about sisters", "[joking] Sibling rivalry.", off)
        self.assertFalse(os.path.exists(os.path.join(self.vault, "Jokes", datetime.date.today().strftime("%Y-%m") + ".md")) and "Sibling" in open(os.path.join(self.vault, "Jokes", datetime.date.today().strftime("%Y-%m") + ".md")).read())
        with open(os.path.join(self.vault, "Talks", datetime.date.today().strftime("%Y-%m") + ".md")) as f:
            self.assertNotIn("back home", f.read())

    def test_a_laugh_turns_the_line_before_into_an_inside_joke_once(self):
        state = os.path.join(self.d, "conv-x.session")
        self.m.save_turn(state, "why is the fan loud", "[joking] It is doing cardio for both of us.")
        self.assertEqual(os.stat(state + ".last").st_mode & 0o777, 0o600)
        prev = self.m.last_turn(state)
        self.m.note_laugh(prev, self.env)
        self.m.note_laugh(prev, self.env)
        with open(os.path.join(self.vault, "Entities", "Inside jokes.md")) as f:
            t = f.read()
        self.assertEqual(t.count("cardio for both of us"), 1)
        self.assertIn('they laughed when I said "It is doing cardio for both of us." (to: why is the fan loud)', t)
        self.assertIn("Sloth plus Mothra", t)


if __name__ == "__main__":
    unittest.main()

    def test_her_own_voice_coming_back_is_not_a_question(self):
        prev = "[gentle] It is four in the afternoon, Matt. The sun is still up and the garden looks fine."
        self.assertTrue(self.m.is_echo("it is four in the afternoon Matt the sun is still up", prev))
        self.assertTrue(self.m.is_echo("The sun is still up and the garden looks fine", prev))
        self.assertFalse(self.m.is_echo("what about the garden", prev), "a short real question that shares a word is not an echo")
        self.assertFalse(self.m.is_echo("is the sun up", ""))

    def test_the_answer_is_said_sentence_by_sentence_as_it_is_written(self):
        st = self.m.Streamer("what time is it", chat=True)
        got = []
        for ch in "<think>\nplanning.\n</think>\n\nIt is four in the afternoon, Matt. The sun is still up! Want me to set a reminder? Is there anything else I can help with?":
            got += st.feed(ch)
        self.assertEqual(got, ["It is four in the afternoon, Matt.", "The sun is still up!"], "a sentence is out as soon as it is complete, offers are not")
        self.assertEqual(st.finish(), [])
        self.assertEqual(st.text(), "It is four in the afternoon, Matt.\nThe sun is still up!")
        st = self.m.Streamer("", chat=False)
        got = []
        for ch in "First. Second one... and a third? Yes":
            got += st.feed(ch)
        self.assertEqual(got, ["First.", "Second one...", "and a third?"])
        self.assertEqual(st.finish(), ["Yes"], "the end finishes the last sentence")
        st = self.m.Streamer("", chat=True)
        self.assertEqual(st.feed("Want me to help with that?", final=True), ["Want me to help with that?"], "an offer that is the whole answer stays")

    def test_a_word_or_two_is_routed_without_the_model(self):
        self.assertEqual(self.m.route("why?", {}), ["chat"])
        self.assertEqual(self.m.route("wifi down", {}), ["network"])
