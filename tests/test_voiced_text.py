"""The text side of labyrinth-voiced (cleaning, sentences, pronunciations, delivery tags). Run: python tests/test_voiced_text.py"""
import importlib.machinery
import importlib.util
import os
import random
import unittest

PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin", "labyrinth-voiced")
loader = importlib.machinery.SourceFileLoader("voiced_text", PATH)
spec = importlib.util.spec_from_loader("voiced_text", loader)
v = importlib.util.module_from_spec(spec)
loader.exec_module(v)


def sentences(text):
    return [v.take_delivery(s) for s in v.split_sentences(v.clean(v.mark_delivery(text)))]


class Voiced(unittest.TestCase):
    def test_other_tags_markup_and_emoji_are_dropped(self):
        self.assertEqual(v.clean("Okay [chuckle] **so** it's `fine` 🐍 <think>x</think>!"), "Okay so it's fine x!")

    def test_names_are_respelled_so_they_are_said_right(self):
        self.assertEqual(v.clean("Sarina and Zlothra's Heimdall"), "Suhreenuh and Zlawthruh's Haymdawl")

    def test_a_delivery_tag_belongs_to_its_sentence_and_is_never_joined_to_another(self):
        got = sentences("[joking] My script ate its own log. Very efficient. [unsure] I think the file is gone. [gentle] That is rough. [serious] Back it up.")
        self.assertEqual([n for n, _ in got], ["joking", "", "unsure", "gentle", "serious"])
        self.assertEqual(got[2][1], "I think the file is gone.")

    def test_long_text_is_cut_and_short_sentences_join_when_untagged(self):
        self.assertEqual(sentences("Hi. Okay."), [("", "Hi. Okay.")])
        self.assertTrue(all(len(p) <= 260 for _, p in sentences("word, " * 120)))

    def test_small_sounds_only_where_they_fit_and_only_sometimes(self):
        joke = [v.dress("Very efficient.", "joking", random.Random(i)) for i in range(40)]
        self.assertTrue(any(j.endswith("[chuckle]") for j in joke) and any(not j.endswith("[chuckle]") for j in joke))
        unsure = {v.dress("I think it is gone.", "unsure", random.Random(i)) for i in range(40)}
        self.assertTrue(any(u.startswith(("Hmm... ", "Um... ", "Well... ")) for u in unsure))
        self.assertEqual(v.dress("It is gone.", "serious", random.Random(1)), "It is gone.")      # nothing added when serious
        self.assertEqual(v.dress("Stop it.", "urgent", random.Random(1)), "Stop it!")
        self.assertEqual(v.dress("Plain.", "", random.Random(1)), "Plain.")
        # a chuckle never comes from her own text
        self.assertNotIn("chuckle", v.clean(v.mark_delivery("She said [chuckle] and [laugh].")))

    def test_every_delivery_has_settings(self):
        for name in ["joking", "unsure", "urgent", "gentle", "serious"]:
            t, before, after = v.DELIVERY[name]
            self.assertTrue(0.4 <= t <= 1.0 and before <= 1500 and after <= 1500)


if __name__ == "__main__":
    unittest.main()
