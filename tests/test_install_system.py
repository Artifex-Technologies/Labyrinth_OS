"""Tests for Labyrinth's side of the installer: its integration file (labyrinth-installer.conf), the browser and optional-part
lists, and labyrinth-first-components. The install program itself is Yggdrasil's yggdrasil-install-system (since 2026-11-07;
its own tests are in Yggdrasil's packages/install-system); the tests that run it load the Python script it replaced, which
Yggdrasil kept for a rollback until 2026-11-15, from git, so they need a Yggdrasil checkout with that commit next to this
repository (or YGGDRASIL_DIR) and are skipped without one. python tests/test_install_system.py"""
import os
import re
import subprocess
import sys
import types
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
AIROOTFS = os.path.join(ROOT, "profile", "airootfs")
CONF = os.path.join(AIROOTFS, "usr", "share", "yggdrasil", "integrations", "labyrinth-installer.conf")
YGG = os.environ.get("YGGDRASIL_DIR") or os.path.join(os.path.dirname(ROOT), "Yggdrasil")
# the Python script its Rust port (packages/install-system) replaced on 2026-11-11; the two do the same (Yggdrasil's
# packages/install-system/parity/compare.py), and the script can be loaded here. Yggdrasil kept it for a rollback until
# 2026-11-15; it comes from git, the last commit that had it (CI fetches that commit into its shallow clone).
YGG_REV = "f9c76a5b6af82198f91b242a4460628d41770599"
TOOL = "packages/installer/helpers/python/install-system"
inst = None
try:
    source = subprocess.run(["git", "-C", YGG, "show", f"{YGG_REV}:{TOOL}"], capture_output=True, check=True).stdout
except (OSError, subprocess.CalledProcessError):
    source = None
if source:
    inst = types.ModuleType("install_system")
    sys.modules["install_system"] = inst
    exec(compile(source, TOOL, "exec"), inst.__dict__)
# labyrinth-tools' Rust program (the image runs /usr/bin/labyrinth-first-components)
FIRST = os.path.join(ROOT, "packages", "tools", "src", "first_components.rs")
needs_program = unittest.skipUnless(inst, f"no Yggdrasil checkout with commit {YGG_REV[:7]} at {YGG}")


def conf_values(path=CONF):
    values = {}
    for line in open(path).read().splitlines():
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
        # the window fetches the install's downloads beforehand while the choices are made (Yggdrasil's INSTALLER.md)
        self.assertEqual(c["prefetch"], "yes")

    def test_the_first_login_list_is_where_labyrinth_first_components_reads_it(self):
        first = open(FIRST).read()
        self.assertIn(conf_values()["first_login"], first)
        # its unit starts only while the list is there
        unit = open(os.path.join(ROOT, "packages", "tools", "units", "user", "labyrinth-first-components.service")).read()
        self.assertIn("ConditionPathExists=" + conf_values()["first_login"] + "\n", unit)

    def test_the_installed_system_offers_the_same_device_list(self):
        # the installer integration is removed from the installed system (labyrinth-install-cleanup); the packages one stays and
        # names the same files for Yggdrasil's device offers
        packages = conf_values(os.path.join(AIROOTFS, "usr", "share", "yggdrasil", "integrations", "labyrinth-packages.conf"))
        c = conf_values()
        self.assertEqual(packages["devices"], c["extras"])
        self.assertEqual(packages["first_login"], c["first_login"])
        self.assertEqual(packages["components"], c["components"])

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

    @needs_program
    def test_the_live_account_is_the_one_the_installer_knows(self):
        # the install hands a saved network only the live account could use (permissions=user:live:;) to the new account: the
        # live medium's account must be the one it looks for
        sysusers = open(os.path.join(AIROOTFS, "etc", "sysusers.d", "live.conf")).read().splitlines()
        self.assertEqual([l.split()[1] for l in sysusers if l.startswith("u ")], [inst.LIVE_USER])
        self.assertIn(inst.LIVE_USER, inst.RESERVED)

    def test_usb_wifi_adapters_have_their_firmware_on_the_medium(self):
        # the installer's Network step joins Wi-Fi on the live medium, and the installed system is a copy of it: the firmware
        # the in-kernel drivers of common USB adapters load (mt76: MediaTek; rtw88 and rtl8xxxu: Realtek; ath9k_htc: Atheros)
        # comes with linux-firmware (all its parts) or must be named part by part
        with open(os.path.join(ROOT, "profile", "packages.x86_64"), encoding="utf-8") as f:
            iso = set(f.read().split())
        parts = {"linux-firmware-mediatek", "linux-firmware-realtek", "linux-firmware-atheros"}
        self.assertTrue("linux-firmware" in iso or parts <= iso, parts - iso)
        self.assertFalse({p for p in iso if "dkms" in p}, "no driver built on the machine on the medium")


CATALOG = os.path.join(AIROOTFS, "usr", "share", "labyrinth", "installer-browsers.json")


@needs_program
class Browsers(unittest.TestCase):
    def setUp(self):
        self.cat = inst.browser_catalog(CATALOG)

    def test_the_offered_browsers_are_the_linux_ones(self):
        ids = [b["id"] for b in self.cat]
        self.assertEqual(ids, ["vivaldi", "firefox", "brave", "librewolf", "mullvad-browser", "waterfox"])
        self.assertEqual([b["id"] for b in self.cat if b.get("chosen")], ["vivaldi"], "Vivaldi is chosen to start with")
        for b in self.cat:
            self.assertTrue(b.get("packages") or b.get("flatpak"), b["id"])
            self.assertTrue(b.get("desktop"), b["id"])
        self.assertEqual(inst.browser_catalog("/nonexistent.json"), [])

    def test_a_choice_removes_the_others_and_installs_its_own(self):
        p = inst.browser_plan(self.cat, ["librewolf", "brave"])
        self.assertIn("vivaldi", p["remove"])
        self.assertIn("vivaldi-ffmpeg-codecs", p["remove"])
        self.assertIn("firefox", p["remove"])
        self.assertEqual(p["install"], ["brave-bin", "librewolf"])
        self.assertEqual(p["flatpaks"], [], "no browser on the list comes from Flathub")
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
            ids = e.get("chosen_if_usb") or []
            self.assertTrue(ids and all(re.fullmatch(r"[0-9a-f]{4}(:[0-9a-f]{4})?", v) for v in ids), e["id"])
            self.assertEqual(len(ids), len(set(ids)), e["id"])
            self.assertNotIn("chosen_if_exists", e, "device software is never on the medium")
        # a maker's id counts only for its kind of device where the maker makes other things too (Yggdrasil's device_offers)
        kinds = {e["id"]: e.get("usb_kind") for e in devices}
        self.assertEqual(kinds, {"logitech": "input", "gaming-mice": "mouse", "razer": None, "ckb-next": "input",
                                 "xbox-controllers": "gamepad", "hp-printers": "printer"},
                         "a Corsair fan controller is no keyboard, a HyperX mouse (HP's id) no printer, a webcam no mouse")
        mice = next(e for e in devices if e["id"] == "gaming-mice")["chosen_if_usb"]
        self.assertTrue({"046d", "1038", "1e7d", "0b05", "258a:0036"} <= set(mice), "libratbag's makers")
        # nothing of it is on the medium, so choosing it queues it for the first login (online)
        p = inst.extras_plan(self.cat, ["razer"], self.comps, set())
        self.assertEqual(p["queue"], ["razer"])
        razer = self.comps["razer"]["packages"]
        self.assertTrue({"dkms", "linux-cachyos-headers", "linux-cachyos-lts-headers"} <= set(razer), "a driver built on the machine brings what it needs")

    def test_laptop_software_is_online_and_ticked_by_the_hardware(self):
        laptop = {e["id"]: e for e in self.cat if e.get("group") == "This laptop"}
        self.assertEqual(set(laptop), {"fingerprint", "thermald", "on-screen-keyboard"})
        with open(os.path.join(ROOT, "profile", "packages.x86_64"), encoding="utf-8") as f:
            iso = set(f.read().split())
        for e in laptop.values():
            self.assertNotIn("chosen_if_exists", e, "laptop software is never on the medium")
            for c in e["components"]:
                self.assertFalse(set(self.comps[c].get("packages", []) + self.comps[c].get("aur", [])) & iso, c)
        # the fingerprint readers libfprint drives, by exact device: Synaptics (06cb) and Goodix (27c6) also make readers it cannot
        fp = laptop["fingerprint"]["chosen_if_usb"]
        self.assertTrue(all(re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{4}", v) for v in fp), "vendor:product, lowercase hex")
        self.assertEqual(len(fp), len(set(fp)))
        self.assertIn("06cb:00fc", fp)
        self.assertNotIn("06cb:00a2", fp, "a Synaptics reader on libfprint's unsupported list")
        self.assertEqual(laptop["thermald"]["chosen_if_machine"],
                         {"cpu_vendor": "GenuineIntel", "battery": True, "absent": "/sys/devices/platform/thinkpad_acpi/dytc_lapmode"},
                         "Intel laptops, except ThinkPads whose firmware runs the cooling (thermald stops there by design)")
        self.assertEqual(self.comps["thermald"]["enable_system"], ["thermald.service"])
        self.assertEqual(laptop["on-screen-keyboard"]["chosen_if_machine"], {"tablet_switch": True})
        self.assertEqual(self.comps["on-screen-keyboard"]["aur"], ["wvkbd"])
        self.assertFalse(self.comps["fingerprint"].get("enable_system") or self.comps["fingerprint"].get("after_install"),
                         "installing fprintd changes nothing in PAM: fingerprint sign-in is the owner's switch in Settings")
        p = inst.extras_plan(self.cat, ["fingerprint", "thermald", "on-screen-keyboard"], self.comps, set())
        self.assertEqual(p["queue"], ["fingerprint", "thermald", "on-screen-keyboard"], "all at the first login, online")

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


if __name__ == "__main__":
    unittest.main()
