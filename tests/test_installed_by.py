"""Every package the [labyrinth] repository publishes is installed by something (LAB-48): the ISO's list, a component (what
the installer and Agora install), Agora's catalog, or another recipe's depends or optdepends. A recipe nothing installs is
published to no one: labyrinth-ollama-tuning and labyrinth-distress were found that way on 2026-11-20. The recipes are this
repository's and Yggdrasil's (YGGDRASIL_DIR, else ../Yggdrasil; left out when it is not there).
Run: python tests/test_installed_by.py"""
import json
import os
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
YGG = Path(os.environ.get("YGGDRASIL_DIR", ROOT.parent / "Yggdrasil"))
SHARE = ROOT / "profile" / "airootfs" / "usr" / "share" / "labyrinth"

# Published, but installed some other way; each with its reason.
ELSEWHERE = {
    # LykanOS's app: LykanOS's own Agora catalog lists it and installs it from [labyrinth]
    "argus-git": "LykanOS's Agora",
    # Limbo's analysis guest image installs YARA-X's scanner from [labyrinth]; nothing on this computer needs it
    "yara-x": "Limbo's analysis image",
    # the own compositor is tried by hand (packages/compositor/session/install-session.sh) until it becomes the session
    "yggdrasil-compositor": "tried by hand until it is the session",
}


def array(text, name):
    """The words of a bash array `name=(...)` (and `name+=(...)`), quotes and comments dropped, `pkg: why` and `pkg>=1` cut to
    the name. A ')' inside quotes does not end the array."""
    out = []
    for m in re.finditer(r"(?m)^\s*" + name + r"\+?=\(", text):
        i, word, quote, words = m.end(), "", None, []
        while i < len(text):
            c = text[i]
            if quote:
                if c == quote:
                    quote = None
                else:
                    word += c
            elif c in "'\"":
                quote = c
            elif c == "#" and not word:
                i = text.find("\n", i)
                i = len(text) if i < 0 else i
                continue
            elif c.isspace() or c == ")":
                if word:
                    words.append(word)
                    word = ""
                if c == ")":
                    break
            else:
                word += c
            i += 1
        out += [w for w in (re.split(r"[<>=:]", w)[0].strip() for w in words) if w]
    return out


def recipes():
    """(folder, names it builds, names it provides, what it pulls in: depends and optdepends, what it replaces)."""
    out = []
    for f in sorted((ROOT / "packages" / "pkgbuilds").glob("*/PKGBUILD")) + sorted((YGG / "packages" / "pkgbuilds").glob("*/PKGBUILD")):
        t = f.read_text(encoding="utf-8")
        m = re.search(r"(?m)^pkgname=(\(.*?\)|\S+)", t, re.S)
        names = [n.strip("'\"") for n in re.findall(r"[^\s()]+", m.group(1))] if m else []
        out.append((f.parent, names, array(t, "provides"), array(t, "depends") + array(t, "optdepends"), array(t, "replaces")))
    return out


def installers(all_recipes):
    """Every name something installs."""
    names = set()
    for f in ("packages.x86_64", "packages.base"):
        names |= {l.strip() for l in (ROOT / "profile" / f).read_text(encoding="utf-8").splitlines() if l.strip() and not l.startswith("#")}
    for c in json.loads((SHARE / "components.json").read_text(encoding="utf-8"))["components"]:
        names |= set(c.get("packages", []))
        if c.get("pkgbuild"):
            names.add(c["pkgbuild"])
    for a in json.loads((SHARE / "agora" / "catalog.json").read_text(encoding="utf-8"))["apps"]:
        s = a.get("source", {})
        if s.get("type") in ("pacman", "aur") and s.get("package"):
            names.add(s["package"])
    for _, _, _, pulls, _ in all_recipes:
        names |= set(pulls)
    return names


class InstalledBy(unittest.TestCase):
    def test_every_published_package_is_installed_by_something(self):
        all_recipes = recipes()
        wanted = installers(all_recipes)
        lonely = []
        for folder, names, provides, _, replaces in all_recipes:
            for n in names:
                # a recipe that replaces a package is installed by pacman -Syu wherever that package is (coreutils, sudo)
                if n in ELSEWHERE or n in wanted or any(p in wanted for p in provides) or replaces:
                    continue
                lonely.append(f"{n} ({folder.relative_to(folder.parent.parent.parent.parent)})")
        self.assertEqual(lonely, [], "published but nothing installs it: put it on the ISO, in a component, in Agora or in a "
                                     "recipe's depends or optdepends, or delete the recipe")

    def test_the_exceptions_are_still_recipes(self):
        built = {n for _, names, _, _, _ in recipes() for n in names}
        if YGG.is_dir():
            self.assertEqual(sorted(set(ELSEWHERE) - built), [], "an exception for a recipe that is gone")

    def test_the_array_reader(self):
        t = "depends=('a' \"b>=2\"\n  c # a comment\n)\noptdepends=('d: why (with words)' 'e: more')\ndepends+=(f)\n"
        self.assertEqual(array(t, "depends"), ["a", "b", "c", "f"])
        self.assertEqual(array(t, "optdepends"), ["d", "e"])


if __name__ == "__main__":
    unittest.main()
