"""labyrinth-sarina-checkup's line in the day's note goes above the status that `sarina daily-report` keeps there, never after it.
Run: python tests/test_sarina_checkup_daily.py"""
import datetime
import importlib.machinery
import importlib.util
import os
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load():
    path = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-sarina-checkup")
    loader = importlib.machinery.SourceFileLoader("checkup", path)
    mod = importlib.util.module_from_spec(importlib.util.spec_from_loader("checkup", loader))
    loader.exec_module(mod)
    return mod


class DailyNote(unittest.TestCase):
    def test_a_new_note_gets_a_header_and_the_line(self):
        vault = tempfile.mkdtemp()
        load().note(vault, "all fine")
        today = datetime.date.today().isoformat()
        text = open(os.path.join(vault, "Daily", today + ".md")).read()
        self.assertTrue(text.startswith("---\ncreated: %s\nsource: labyrinth-sarina-checkup\ntags: [daily]\n---\n\n# %s\n\n- " % (today, today)))
        self.assertIn("check-up: all fine\n", text)

    def test_the_line_goes_above_the_status_sections(self):
        vault = tempfile.mkdtemp()
        today = datetime.date.today().isoformat()
        os.makedirs(os.path.join(vault, "Daily"))
        p = os.path.join(vault, "Daily", today + ".md")
        with open(p, "w") as f:
            f.write("---\ncreated: %s\n---\n\n# %s\n\n- 10:00 check-up: all fine\n\n## Status (12:00)\n\n- Load: 0.5\n\n## Numbers\n\n- 12:00 | disk / 40%% | crashes 0 | failed 0 | memory 30%% | load 0.5\n" % (today, today))
        load().note(vault, "spotify crashed 2 times")
        text = open(p).read()
        self.assertLess(text.index("10:00 check-up"), text.index("spotify crashed"))
        self.assertLess(text.index("spotify crashed"), text.index("## Status (12:00)"))
        self.assertTrue(text.endswith("load 0.5\n"))
        self.assertEqual(text.count("## Status"), 1)


if __name__ == "__main__":
    unittest.main()
