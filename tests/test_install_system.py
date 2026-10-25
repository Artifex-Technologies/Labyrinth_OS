"""Tests for labyrinth-install-system's pure parts. python tests/test_install_system.py"""
import importlib.machinery
import importlib.util
import os
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOL = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-install-system")
loader = importlib.machinery.SourceFileLoader("install_system", TOOL)
spec = importlib.util.spec_from_loader("install_system", loader)
inst = importlib.util.module_from_spec(spec)
loader.exec_module(inst)

MODEL_MAP = """# consolelayout\txlayout\txmodel\txvariant\txoptions\tbcp47
uk\tgb\tpc105\t-\tterminate:ctrl_alt_bksp\ten-GB
us\tus\tpc105+inet\t-\tterminate:ctrl_alt_bksp\ten-US,en
dvorak\tus\tpc105\tdvorak\tterminate:ctrl_alt_bksp\t-
es-dvorak\tes\tmicrosoftpro\tdvorak\tterminate:ctrl_alt_bksp\t-
"""


class ConsoleKeymap(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp()
        os.makedirs(os.path.join(self.root, "usr/share/systemd"))
        open(os.path.join(self.root, "usr/share/systemd/kbd-model-map"), "w").write(MODEL_MAP)
        for name in ("colemak", "fr"):
            d = os.path.join(self.root, "usr/share/kbd/keymaps/i386", name)
            os.makedirs(d, exist_ok=True)
            open(os.path.join(d, name + ".map.gz"), "w").close()

    def test_the_console_gets_its_own_name_for_a_layout(self):
        k = lambda layout, variant="": inst.console_keymap(layout, variant, self.root)
        self.assertEqual(k("gb"), "uk")
        self.assertEqual(k("us", "dvorak"), "dvorak")
        self.assertEqual(k("es", "dvorak"), "es-dvorak")
        self.assertEqual(k("us", "colemak"), "colemak", "a console keymap named after the variant")
        self.assertEqual(k("fr"), "fr", "not in the table: a console keymap of the same name")
        self.assertEqual(k("zz"), "us", "nothing fits")

    def test_a_variant_is_checked_like_a_layout(self):
        plan = {"disk": "/dev/vda", "mode": "erase", "encrypt": False, "user": {"name": "A", "login": "a", "password": "longenough"},
                "hostname": "x", "timezone": "UTC", "locale": "en_US.UTF-8", "keymap": "us", "keymap_variant": "dvorak; rm"}
        self.assertEqual(inst.check_plan(plan), "unknown keyboard variant")


GB = 1024**3
MIB = 1024**2
# a 500 GB disk with Windows on it: its EFI partition, Windows itself, and 200 GB free at the end
DISK = {"path": "/dev/vda", "size": 500 * GB, "table": "gpt", "partitions": [
    {"path": "/dev/vda1", "number": 1, "start": MIB, "size": 300 * MIB, "fstype": "vfat", "label": "EFI", "efi": True, "mounted": False},
    {"path": "/dev/vda2", "number": 2, "start": 301 * MIB, "size": 299 * GB, "fstype": "ntfs", "label": "Windows", "efi": False, "mounted": False}]}


def manual(**kw):
    p = {"disk": "/dev/vda", "mode": "manual", "encrypt": False, "user": {"name": "A", "login": "a", "password": "longenough"},
         "hostname": "x", "timezone": "UTC", "locale": "en_US.UTF-8", "keymap": "us",
         "create": [{"id": "r", "start": 300 * GB, "size": 150 * GB, "kind": "linux"}],
         "use": [{"part": "/dev/vda1", "mount": "/boot/efi", "format": None}, {"part": "new:r", "mount": "/", "format": "btrfs"}]}
    p.update(kw)
    return p


class Manual(unittest.TestCase):
    def test_free_space_lies_between_the_partitions(self):
        gaps = inst.free_space([(q["start"], q["size"]) for q in DISK["partitions"]], MIB, DISK["size"] - MIB)
        self.assertEqual(len(gaps), 1)
        self.assertGreaterEqual(gaps[0]["start"], 301 * MIB + 299 * GB)
        self.assertEqual(gaps[0]["start"] % MIB, 0)
        self.assertEqual(inst.free_space([], MIB, 100 * MIB), [{"start": MIB, "size": 99 * MIB}])
        self.assertEqual(inst.free_space([(2 * MIB, 10 * MIB)], MIB, 12 * MIB), [], "a 1 MiB sliver is not free space")

    def test_beside_windows_on_its_own_efi_partition(self):
        p = manual()
        self.assertIsNone(inst.check_plan(p))
        self.assertIsNone(inst.manual_fits(p, DISK))

    def test_what_a_plan_needs(self):
        self.assertIn("exactly one", inst.check_plan(manual(use=[{"part": "/dev/vda1", "mount": "/boot/efi", "format": None}])))
        self.assertIn("EFI", inst.check_plan(manual(use=[{"part": "new:r", "mount": "/", "format": "btrfs"}])))
        self.assertIn("formatted", inst.check_plan(manual(use=[{"part": "/dev/vda1", "mount": "/boot/efi", "format": None},
                                                               {"part": "/dev/vda2", "mount": "/", "format": None}])))
        self.assertIn("cannot be formatted as", inst.check_plan(manual(use=[{"part": "/dev/vda1", "mount": "/boot/efi", "format": "ext4"},
                                                                            {"part": "new:r", "mount": "/", "format": "btrfs"}])))
        self.assertIn("mount point", inst.check_plan(manual(use=[{"part": "/dev/vda1", "mount": "/etc", "format": None}])))

    def test_against_the_disk(self):
        self.assertIn("does not fit", inst.manual_fits(manual(create=[{"id": "r", "start": 200 * GB, "size": 150 * GB, "kind": "linux"}]), DISK),
                      "overlaps Windows")
        self.assertIn("at least", inst.manual_fits(manual(create=[{"id": "r", "start": 300 * GB, "size": 10 * GB, "kind": "linux"}]), DISK))
        # deleting Windows frees its space
        p = manual(delete=[2], create=[{"id": "r", "start": 301 * MIB, "size": 400 * GB, "kind": "linux"}])
        self.assertIsNone(inst.manual_fits(p, DISK))
        busy = dict(DISK, partitions=[dict(DISK["partitions"][0], mounted=True), DISK["partitions"][1]])
        self.assertIn("in use", inst.manual_fits(manual(), busy))
        self.assertIn("no partition table", inst.manual_fits(manual(), dict(DISK, table="", partitions=[])))


CATALOG = os.path.join(ROOT, "profile", "airootfs", "usr", "share", "labyrinth", "installer-browsers.json")


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
FIRST = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-first-components")


class Extras(unittest.TestCase):
    def setUp(self):
        self.cat = inst.extras_catalog(EXTRAS)
        self.comps = inst.components(COMPONENTS)

    def test_sarina_and_heimdall_are_offered_as_components(self):
        self.assertEqual([e["id"] for e in self.cat], ["sarina", "heimdall"])
        for e in self.cat:
            self.assertTrue(e["components"] and all(c in self.comps for c in e["components"]), e["id"])
            self.assertFalse(e.get("chosen"), "never chosen outright: only when the medium carries it")
            self.assertTrue(e.get("chosen_if_exists", "").startswith("/usr/bin/"), e["id"])
        self.assertEqual(inst.extras_catalog("/nonexistent.json"), [])

    def test_on_the_medium_kept_or_removed_otherwise_left_for_the_first_login(self):
        everything = {"sarina-git", "heimdall-git"}
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
