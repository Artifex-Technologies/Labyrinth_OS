#!/usr/bin/env python3
"""labyrinth-system-files owns the ISO's own files (scripts/system-files.sh, profile/not-packaged.txt), so that an update
refreshes them on an installed system. A file it owns that another recipe also ships would stop mkarchiso's pacstrap and
the package's install ("exists in filesystem"); a rule in the list that matches nothing is a leftover."""

import fnmatch
import os
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
YGG = Path(os.environ.get("YGGDRASIL_DIR", ROOT.parent / "Yggdrasil"))
AIROOTFS = ROOT / "profile" / "airootfs"
OWN = ROOT / "packages" / "pkgbuilds" / "labyrinth-system-files"


def listed(kind=None):
    args = [str(ROOT / "scripts" / "system-files.sh")] + ([kind] if kind else [])
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout.split()


def rules():
    lines = (ROOT / "profile" / "not-packaged.txt").read_text(encoding="utf-8").splitlines()
    return [l.strip() for l in lines if l.strip() and not l.strip().startswith("#")]


def every_file():
    return sorted("/" + str(p.relative_to(AIROOTFS)) for p in AIROOTFS.rglob("*") if p.is_file() or p.is_symlink())


class SystemFiles(unittest.TestCase):
    def test_what_it_owns_and_what_it_leaves(self):
        packaged = listed()
        for f in ("/usr/share/labyrinth/agora/catalog.json", "/usr/share/labyrinth/components.json",
                  "/usr/share/yggdrasil/integrations/labyrinth-security.conf", "/etc/yggdrasil/dock"):
            self.assertIn(f, packaged)
        for f in ("/etc/greetd/live.toml", "/etc/sudoers.d/00-live-nopasswd", "/usr/share/yggdrasil/integrations/labyrinth-installer.conf",
                  "/etc/pacman.conf", "/etc/os-release", "/usr/share/polkit-1/actions/org.labyrinth.distress.policy"):
            self.assertNotIn(f, packaged)
        self.assertFalse([f for f in packaged if f.startswith("/home/")], "nothing in a home folder")
        self.assertEqual(listed("recipes"), ["/usr/share/polkit-1/actions/org.labyrinth.distress.policy"])

    def test_every_rule_still_matches_a_file(self):
        files = every_file()
        stale = [r for r in rules() if not fnmatch.filter(files, r.removeprefix("recipe:"))]
        self.assertEqual(stale, [], "a rule in profile/not-packaged.txt for a file that is gone")

    def test_no_other_recipe_ships_a_file_it_owns(self):
        recipes = [p for d in (ROOT / "packages" / "pkgbuilds", AIROOTFS / "usr" / "share" / "labyrinth" / "pkgbuilds", YGG / "packages" / "pkgbuilds")
                   for p in sorted(d.glob("*/PKGBUILD")) if p.parent != OWN]
        texts = {p: p.read_text(encoding="utf-8") for p in recipes}
        both = [f"{f} ({p.parent.name})" for f in listed() for p, t in texts.items() if f in t]
        self.assertEqual(both, [], "mark it recipe: in profile/not-packaged.txt, or take it out of the other recipe")


if __name__ == "__main__":
    unittest.main()
