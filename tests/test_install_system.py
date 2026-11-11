"""Tests for Labyrinth's side of the installer: its integration file (labyrinth-installer.conf), the browser and optional-part
lists, and labyrinth-first-components. The install program itself is Yggdrasil's yggdrasil-install-system (since 2026-11-07;
its own tests are in Yggdrasil's packages/install-system and packages/installer/helpers/tests); the tests that run it load
the Python script Yggdrasil keeps for a rollback, so they need a Yggdrasil checkout next to this repository (or
YGGDRASIL_DIR) and are skipped without one. python tests/test_install_system.py"""
import importlib.machinery
import importlib.util
import os
import re
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
AIROOTFS = os.path.join(ROOT, "profile", "airootfs")
CONF = os.path.join(AIROOTFS, "usr", "share", "yggdrasil", "integrations", "labyrinth-installer.conf")
YGG = os.environ.get("YGGDRASIL_DIR") or os.path.join(os.path.dirname(ROOT), "Yggdrasil")
# the Python script, which Yggdrasil keeps for a rollback since its Rust port (packages/install-system) replaced it on
# 2026-11-11; the two do the same (Yggdrasil's packages/install-system/parity/compare.py), and the script can be loaded here
TOOL = os.path.join(YGG, "packages", "installer", "helpers", "python", "install-system")
inst = None
if os.path.isfile(TOOL):
    loader = importlib.machinery.SourceFileLoader("install_system", TOOL)
    spec = importlib.util.spec_from_loader("install_system", loader)
    inst = importlib.util.module_from_spec(spec)
    loader.exec_module(inst)
# the script, kept for a rollback since labyrinth-tools' Rust program replaced it (the image runs /usr/bin/labyrinth-first-components)
FIRST = os.path.join(ROOT, "packages", "tools", "python", "labyrinth-first-components")
needs_program = unittest.skipUnless(inst, f"no Yggdrasil checkout with the install program at {TOOL}")


def conf_values():
    values = {}
    for line in open(CONF).read().splitlines():
        line = line.split("#", 1)[0].strip()
        if "=" in line:
            k, v = line.split("=", 1)
            values[k.strip()] = v.strip()
    return values


class TheIntegrationFile(unittest.TestCase):
    def test_it_names_yggdrasils_program_and_labyrinths_own_steps(self):
        c = conf_values()
        self.assertEqual(c["kind"], "installer")
        self.assertEqual(c["command"], "/usr/bin/yggdrasil-install-system")
        # Labyrinth's own steps come with the labyrinth-tools package (/usr/bin), which the image installs
        pkgbuild = open(os.path.join(ROOT, "packages", "pkgbuilds", "labyrinth-tools", "PKGBUILD")).read()
        for key in ("cleanup", "finish", "boot_menu_theme"):
            name = os.path.basename(c[key])
            self.assertEqual(c[key], "/usr/bin/" + name, key)
            self.assertRegex(pkgbuild, r"\b%s\b" % re.escape(name), f"{key}: labyrinth-tools installs {name}")
        for key in ("browsers", "extras", "components"):
            self.assertTrue(os.path.isfile(AIROOTFS + c[key]), f"{key}: {c[key]} is on the medium")
        self.assertIn("nvidia_drm.modeset=1", c["kernel_options"].split())
        self.assertEqual(set(c["reserved_logins"].split()), {"heimdall", "sarina"})

    def test_the_first_login_list_is_where_labyrinth_first_components_reads_it(self):
        first = open(FIRST).read()
        self.assertIn(conf_values()["first_login"], first)

    @needs_program
    def test_labyrinth_installs_as_it_did_with_its_own_program(self):
        inst.configure(conf_values(), {"NAME": "Labyrinth OS", "ID": "labyrinth"})
        try:
            self.assertEqual(inst.SFS, "/run/archiso/bootmnt/labyrinth/x86_64/airootfs.sfs", "profiledef.sh's install_dir")
            self.assertEqual((inst.TARGET, inst.MAPPER), ("/mnt/labyrinth-install", "labyrinth-root"))
            self.assertEqual(inst.GRUB_DEFAULTS["GRUB_DISTRIBUTOR"], "Labyrinth")
            self.assertIn("openrazer", inst.GROUPS)
            self.assertEqual(inst.check_plan({"disk": "/dev/vda", "mode": "erase", "user": {"name": "A", "login": "sarina", "password": "x"},
                                              "hostname": "x", "timezone": "UTC", "locale": "en_US.UTF-8", "keymap": "us"}),
                             "that account name is taken by the system")
        finally:
            inst.configure({}, {})


CATALOG = os.path.join(AIROOTFS, "usr", "share", "labyrinth", "installer-browsers.json")


@needs_program
class Browsers(unittest.TestCase):
    def setUp(self):
        self.cat = inst.browser_catalog(CATALOG)

    def test_the_offered_browsers_are_the_linux_ones(self):
        ids = [b["id"] for b in self.cat]
        self.assertEqual(ids, ["vivaldi", "firefox", "brave", "librewolf", "mullvad-browser", "waterfox", "opera", "opera-gx"])
        self.assertEqual([b["id"] for b in self.cat if b.get("chosen")], ["vivaldi"], "Vivaldi is chosen to start with")
        for b in self.cat:
            self.assertTrue(b.get("packages") or b.get("flatpak"), b["id"])
            self.assertTrue(b.get("desktop"), b["id"])
        self.assertEqual(inst.browser_catalog("/nonexistent.json"), [])

    def test_a_choice_removes_the_others_and_installs_its_own(self):
        p = inst.browser_plan(self.cat, ["librewolf", "opera-gx"])
        self.assertIn("vivaldi", p["remove"])
        self.assertIn("vivaldi-ffmpeg-codecs", p["remove"])
        self.assertIn("firefox", p["remove"])
        self.assertEqual(p["install"], ["librewolf"])
        self.assertEqual(p["flatpaks"], ["com.opera.opera-gx"])
        none = inst.browser_plan(self.cat, [])
        self.assertEqual((none["install"], none["flatpaks"]), ([], []))
        self.assertIn("vivaldi", none["remove"])

    def test_the_plan_names_known_browsers_only(self):
        base = {"disk": "/dev/vda", "mode": "erase", "encrypt": False, "user": {"name": "A", "login": "a", "password": "longenough"},
                "hostname": "x", "timezone": "UTC", "locale": "en_US.UTF-8", "keymap": "us"}
        old = inst.BROWSERS_FILE
        inst.BROWSERS_FILE = CATALOG
        try:
            self.assertIsNone(inst.check_plan(base), "no browsers key: the medium's browsers stay")
            self.assertIsNone(inst.check_plan(dict(base, browsers=["brave", "firefox"], default_browser="firefox")))
            self.assertIsNone(inst.check_plan(dict(base, browsers=[])))
            self.assertEqual(inst.check_plan(dict(base, browsers=["duckduckgo"])), "unknown browser chosen")
            self.assertEqual(inst.check_plan(dict(base, browsers="vivaldi")), "unknown browser chosen")
            self.assertEqual(inst.check_plan(dict(base, browsers=["brave"], default_browser="vivaldi")), "the default browser must be one of those chosen")
        finally:
            inst.BROWSERS_FILE = old

    def test_the_default_browser_opens_links_and_sits_in_the_dock(self):
        mime = "# comment\n[Default Applications]\nx-scheme-handler/http=vivaldi-stable.desktop\ntext/html=vivaldi-stable.desktop\nvideo/mp4=yggdrasil-player.desktop\n"
        out = inst.mimeapps_with_browser(mime, "librewolf.desktop")
        self.assertIn("x-scheme-handler/http=librewolf.desktop", out)
        self.assertIn("x-scheme-handler/https=librewolf.desktop", out, "a missing key is added")
        self.assertIn("video/mp4=yggdrasil-player.desktop", out)
        self.assertNotIn("vivaldi", out)
        self.assertIn("text/html=x.desktop", inst.mimeapps_with_browser("", "x.desktop"))
        dock = "# pins\nminos\nvivaldi-stable\nfirefox\nsteam\n"
        ids = {"vivaldi-stable", "firefox", "librewolf"}
        self.assertEqual(inst.dock_with_browser(dock, ids, "librewolf"), "# pins\nminos\nlibrewolf\nsteam\n")
        self.assertEqual(inst.dock_with_browser(dock, ids, None), "# pins\nminos\nsteam\n")


EXTRAS = os.path.join(ROOT, "profile", "airootfs", "usr", "share", "labyrinth", "installer-extras.json")
COMPONENTS = os.path.join(ROOT, "profile", "airootfs", "usr", "share", "labyrinth", "components.json")


@needs_program
class Extras(unittest.TestCase):
    def setUp(self):
        self.cat = inst.extras_catalog(EXTRAS)
        self.comps = inst.components(COMPONENTS)

    def test_sarina_and_heimdall_are_offered_as_components(self):
        ai = [e for e in self.cat if e.get("group") == "AI and security"]
        self.assertEqual([e["id"] for e in ai], ["sarina", "heimdall"])
        for e in self.cat:
            self.assertTrue(e["components"] and all(c in self.comps for c in e["components"]), e["id"])
            self.assertFalse(e.get("chosen"), "never chosen outright: only when the medium carries it or the device is plugged in")
        for e in ai:
            self.assertTrue(e.get("chosen_if_exists", "").startswith("/usr/bin/"), e["id"])
        self.assertEqual(inst.extras_catalog("/nonexistent.json"), [])

    def test_device_software_is_offered_only_online_and_ticked_by_the_device(self):
        devices = [e for e in self.cat if e.get("group") == "Devices"]
        self.assertTrue({"logitech", "gaming-mice", "razer", "xbox-controllers", "hp-printers"} <= {e["id"] for e in devices})
        for e in devices:
            self.assertTrue(e.get("chosen_if_usb") and all(len(v) == 4 for v in e["chosen_if_usb"]), e["id"])
            self.assertNotIn("chosen_if_exists", e, "device software is never on the medium")
        # nothing of it is on the medium, so choosing it queues it for the first login (online)
        p = inst.extras_plan(self.cat, ["razer"], self.comps, set())
        self.assertEqual(p["queue"], ["razer"])
        razer = self.comps["razer"]["packages"]
        self.assertTrue({"dkms", "linux-cachyos-headers", "linux-cachyos-lts-headers"} <= set(razer), "a driver built on the machine brings what it needs")

    def test_on_the_medium_kept_or_removed_otherwise_left_for_the_first_login(self):
        everything = {"sarina-git", "heimdall-git", "clamav"}
        p = inst.extras_plan(self.cat, ["heimdall"], self.comps, everything)
        self.assertEqual((p["remove"], p["keep"], p["queue"]), (["sarina-git"], ["heimdall"], []))
        p = inst.extras_plan(self.cat, ["sarina", "heimdall"], self.comps, set())
        self.assertEqual((p["remove"], p["keep"], p["queue"]), ([], [], ["sarina", "heimdall"]), "an end-user medium: both at the first login")
        p = inst.extras_plan(self.cat, [], self.comps, {"heimdall-git"})
        self.assertEqual((p["remove"], p["keep"], p["queue"]), (["heimdall-git"], [], []))

    def test_the_plan_names_known_parts_only(self):
        base = {"disk": "/dev/vda", "mode": "erase", "encrypt": False, "user": {"name": "A", "login": "a", "password": "longenough"},
                "hostname": "x", "timezone": "UTC", "locale": "en_US.UTF-8", "keymap": "us"}
        old = inst.EXTRAS_FILE
        inst.EXTRAS_FILE = EXTRAS
        try:
            self.assertIsNone(inst.check_plan(dict(base, extras=["sarina"])))
            self.assertIsNone(inst.check_plan(dict(base, extras=[])))
            self.assertEqual(inst.check_plan(dict(base, extras=["steam"])), "unknown optional part chosen")
            self.assertEqual(inst.check_plan(dict(base, extras="sarina")), "unknown optional part chosen")
        finally:
            inst.EXTRAS_FILE = old

    def test_the_first_login_list_is_only_the_new_users(self):
        floader = importlib.machinery.SourceFileLoader("first_components", FIRST)
        fspec = importlib.util.spec_from_loader("first_components", floader)
        first = importlib.util.module_from_spec(fspec)
        floader.exec_module(first)
        d = tempfile.mkdtemp()
        f = os.path.join(d, "list.json")
        open(f, "w").write('{"user": "ada", "components": ["sarina", "heimdall", "bad; rm -rf /"]}')
        self.assertEqual(first.todo(f, "ada"), ["sarina", "heimdall"], "ids only")
        self.assertEqual(first.todo(f, "bob"), [])
        self.assertEqual(first.todo(os.path.join(d, "none.json"), "ada"), [])



if __name__ == "__main__":
    unittest.main()
