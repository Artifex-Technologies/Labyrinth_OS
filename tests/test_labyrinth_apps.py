"""Tests for labyrinth-apps and scripts/gen-packages.py. Run: python tests/test_labyrinth_apps.py"""
import contextlib
import importlib.machinery
import importlib.util
import io
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


apps = load_module("labyrinth_apps", os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-apps"))
gen = load_module("gen_packages", os.path.join(ROOT, "scripts", "gen-packages.py"))
apps.MANIFEST = MANIFEST


def run(argv, installed):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = apps.main(argv, installed=installed)
    return rc, out.getvalue(), err.getvalue()


class Manifest(unittest.TestCase):
    def test_presets_reference_real_components(self):
        comps, presets = apps.load(MANIFEST)
        self.assertIn("everything", presets)
        self.assertEqual(presets["minimal"], [])

    def test_every_component_installs_something(self):
        comps, _ = apps.load(MANIFEST)
        for i, c in comps.items():
            self.assertTrue(apps.all_packages(c), i)

    def test_unknown_preset_member_is_rejected(self):
        import tempfile
        d = json.load(open(MANIFEST, encoding="utf-8"))
        d["presets"]["bad"] = ["nope"]
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(d, f)
        with self.assertRaises(ValueError):
            apps.load(f.name)
        os.unlink(f.name)


class Commands(unittest.TestCase):
    def test_list_shows_state(self):
        rc, out, _ = run(["list"], installed={"obsidian", "rclone"})
        self.assertEqual(rc, 0)
        self.assertRegex(out, r"obsidian\s+installed")
        self.assertRegex(out, r"proton-drive\s+partial")
        self.assertRegex(out, r"steam\s+not installed")

    def test_dry_run_install_skips_installed_and_runs_nothing(self):
        rc, out, _ = run(["install", "obsidian", "spotify", "--dry-run"], installed={"obsidian"})
        self.assertEqual(rc, 0)
        self.assertIn("spotify-launcher", out)
        self.assertNotIn("obsidian", out.split("==>")[1])
        self.assertIn("pacman -S --needed", out)

    def test_pkgbuild_component_uses_makepkg(self):
        rc, out, _ = run(["install", "proton-pass", "--dry-run"], installed=set())
        self.assertIn("makepkg -si", out)
        self.assertIn("proton-pass", out)

    def test_makepkg_from_agora_asks_through_its_password_box(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            os.environ.pop("AGORA_SUDO", None)
            self.assertEqual(apps.makepkg_gui_args(tmp), [])
            os.environ["AGORA_SUDO"] = "/usr/lib/agora/sudo"
            try:
                args = apps.makepkg_gui_args(tmp)
            finally:
                del os.environ["AGORA_SUDO"]
            self.assertEqual(args[:2], ["--noconfirm", "--config"])
            conf = open(args[2]).read()
            self.assertIn("source /etc/makepkg.conf", conf)
            self.assertIn("PACMAN_AUTH=(/usr/lib/agora/sudo)", conf)

    def test_preset_expands(self):
        rc, out, _ = run(["install", "--preset", "gaming", "--dry-run"], installed=set())
        self.assertIn("steam", out)
        self.assertNotIn("spotify", out)

    def test_nothing_to_do_when_all_installed(self):
        rc, out, _ = run(["install", "obsidian", "--dry-run"], installed={"obsidian"})
        self.assertEqual((rc, out.strip()), (0, "nothing to do"))

    def test_unknown_component_and_preset(self):
        self.assertEqual(run(["install", "nope", "--dry-run"], set())[0], 2)
        self.assertEqual(run(["install", "--preset", "nope", "--dry-run"], set())[0], 2)
        self.assertEqual(run(["install", "--dry-run"], set())[0], 2)

    def test_remove_only_touches_installed_packages(self):
        rc, out, _ = run(["remove", "proton-drive", "--dry-run"], installed={"rclone"})
        self.assertIn("pacman -Rs rclone", out)
        self.assertNotIn("fuse3", out)

    def test_weather_is_optional_and_enables_its_timer(self):
        comps, presets = apps.load(MANIFEST)
        self.assertIn("weather", comps)
        self.assertNotIn("weather", presets["minimal"] + presets["gaming"] + presets["creative"])
        rc, out, _ = run(["install", "weather", "--dry-run"], installed=set())
        self.assertIn("makepkg -si", out)
        self.assertIn("systemctl --global enable yggdrasil-weather-record.timer", out)

    def test_removing_weather_disables_the_timer_first(self):
        rc, out, _ = run(["remove", "weather", "--dry-run"], installed={"yggdrasil-weather"})
        self.assertLess(out.index("--global disable"), out.index("pacman -Rs yggdrasil-weather"))

    def test_minimal_iso_has_no_weather_package(self):
        text = gen.compose(gen.read_base(os.path.join(ROOT, "profile", "packages.base")), json.load(open(MANIFEST, encoding="utf-8")), [])
        self.assertNotIn("yggdrasil-weather", text)

    def test_without_pacman_it_explains_instead_of_crashing(self):
        orig = apps.pacman_installed
        apps.pacman_installed = lambda: None
        try:
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                self.assertEqual(apps.main(["install", "obsidian"]), 2)
                self.assertEqual(apps.main(["install", "obsidian", "--dry-run"]), 0)
                self.assertEqual(apps.main(["list"]), 0)
            self.assertIn("pacman not found", err.getvalue())
        finally:
            apps.pacman_installed = orig

    def test_info_mentions_unverified_packages(self):
        rc, out, _ = run(["info", "steam"], installed=set())
        self.assertIn("proton-cachyos", out)


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
