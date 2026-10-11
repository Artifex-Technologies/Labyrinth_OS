import unittest

import os
import sys

# The shipped copy is the only one: profile/airootfs/usr/local/bin/steam_playall.py
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "profile", "airootfs", "usr", "local", "bin"))
from steam_playall import apply  # noqa: E402

BASE = '''"InstallConfigStore"
{
\t"Software"
\t{
\t\t"Valve"
\t\t{
\t\t\t"Steam"
\t\t\t{
\t\t\t\t"CompatToolMapping"
\t\t\t\t{
%s\t\t\t\t}
\t\t\t}
\t\t}
\t}
}
'''
GAME = '\t\t\t\t\t"620"\n\t\t\t\t\t{\n\t\t\t\t\t\t"name"\t\t"GE-Proton9-1"\n\t\t\t\t\t}\n'
ZERO = '\t\t\t\t\t"0"\n\t\t\t\t\t{\n\t\t\t\t\t\t"name"\t\t"proton_9"\n\t\t\t\t\t}\n'


class T(unittest.TestCase):
    def test_adds_to_empty_block(self):
        new, st = apply(BASE % "")
        self.assertEqual(st, "added")
        self.assertIn('"name"\t\t"proton_experimental"', new)
        self.assertEqual(new.count("{"), new.count("}"))

    def test_adds_beside_per_game_entry_and_keeps_it(self):
        new, st = apply(BASE % GAME)
        self.assertEqual(st, "added")
        self.assertIn('"GE-Proton9-1"', new)
        self.assertIn('"proton_experimental"', new)

    def test_existing_default_is_never_changed(self):
        text = BASE % ZERO
        new, st = apply(text)
        self.assertEqual(st, "already")
        self.assertEqual(new, text)

    def test_nested_zero_does_not_count_as_default(self):
        # "0" nested inside another entry is not the default mapping.
        nested = '\t\t\t\t\t"620"\n\t\t\t\t\t{\n\t\t\t\t\t\t"0"\n\t\t\t\t\t\t{\n\t\t\t\t\t\t}\n\t\t\t\t\t}\n'
        self.assertEqual(apply(BASE % nested)[1], "added")

    def test_idempotent(self):
        once, _ = apply(BASE % GAME)
        twice, st = apply(once)
        self.assertEqual(st, "already")
        self.assertEqual(once, twice)

    def test_refuses_without_block(self):
        text = '"InstallConfigStore"\n{\n}\n'
        new, st = apply(text)
        self.assertTrue(st.startswith("refused"))
        self.assertEqual(new, text)

    def test_preserves_crlf(self):
        new, _ = apply((BASE % "").replace("\n", "\r\n"))
        self.assertNotIn("\r\n\n", new)
        self.assertNotRegex(new, r"(?<!\r)\n")


if __name__ == "__main__":
    unittest.main()
