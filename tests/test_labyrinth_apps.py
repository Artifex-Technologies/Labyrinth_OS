"""Tests for scripts/gen-packages.py (the ISO's package list from packages.base and components.json). labyrinth-apps itself is
labyrinth-tools' Rust program, tested in packages/tools (src/apps/tests.rs, tests/apps.rs); its Python script and the tests of it
here were removed on 2026-11-15. Run: python tests/test_labyrinth_apps.py"""
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "profile", "airootfs", "usr", "share", "labyrinth", "components.json")


def load_module(name, path):
    loader = importlib.machinery.SourceFileLoader(name, path)
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


gen = load_module("gen_packages", os.path.join(ROOT, "scripts", "gen-packages.py"))


class Generator(unittest.TestCase):
    def setUp(self):
        self.data = json.load(open(MANIFEST, encoding="utf-8"))
        self.base = gen.read_base(os.path.join(ROOT, "profile", "packages.base"))

    def test_minimal_has_no_component_packages(self):
        text = gen.compose(self.base, self.data, [])
        self.assertNotIn("\nsteam\n", text)
        self.assertNotIn("obsidian", text)
        self.assertIn("\nhyprland\n", text)

    def test_components_add_their_packages_once(self):
        text = gen.compose(self.base, self.data, ["steam", "obsidian", "steam"])
        self.assertEqual(text.count("\nsteam\n"), 1)
        self.assertIn("\nobsidian\n", text)

    def test_minimal_iso_has_no_weather_package(self):
        text = gen.compose(self.base, self.data, [])
        self.assertNotIn("yggdrasil-weather", text)

    def test_unknown_component_fails(self):
        with self.assertRaises(KeyError):
            gen.compose(self.base, self.data, ["nope"])

    def test_no_trailing_comments_or_spaces(self):
        text = gen.compose(self.base, self.data, self.data["presets"]["everything"])
        for line in text.splitlines():
            if not line.startswith("#"):
                self.assertNotRegex(line, r"[#\s]")

    def test_base_rejects_trailing_comment(self):
        import tempfile
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("good\nbad # trailing\n")
        with self.assertRaises(ValueError):
            gen.read_base(f.name)
        os.unlink(f.name)

    def test_committed_package_list_is_current(self):
        rc = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", "gen-packages.py"), "--check"]).returncode
        self.assertEqual(rc, 0)


if __name__ == "__main__":
    unittest.main()
