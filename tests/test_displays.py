"""Tests for labyrinth-displays with `hyprctl monitors all -j` shaped fixtures. python tests/test_displays.py"""
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOL = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin", "labyrinth-displays")
RULES = os.path.join(ROOT, "profile", "airootfs", "usr", "share", "labyrinth", "displays.json")

loader = importlib.machinery.SourceFileLoader("labyrinth_displays", TOOL)
spec = importlib.util.spec_from_loader("labyrinth_displays", loader)
disp = importlib.util.module_from_spec(spec)
loader.exec_module(disp)

# Shapes follow `hyprctl monitors all -j`. Connector names and the exact mode lists are
# invented for the test: the real ones are only known on the machine.
ULTRAWIDE = {
    "name": "DP-1", "description": "Acer Technologies X34 GS #ABC123", "make": "Acer Technologies", "model": "X34 GS",
    "width": 3440, "height": 1440, "refreshRate": 143.97, "x": 0, "y": 0, "scale": 1.0, "disabled": False,
    "availableModes": ["3440x1440@143.97Hz", "3440x1440@120.00Hz", "3440x1440@100.00Hz", "3440x1440@59.97Hz", "2560x1440@143.97Hz"],
}
DUALUP = {
    "name": "HDMI-A-1", "description": "LG Electronics LG SDQHD #0x00000001", "make": "LG Electronics", "model": "LG SDQHD",
    "width": 2560, "height": 2880, "refreshRate": 59.98, "x": 3440, "y": 0, "scale": 1.0, "disabled": False,
    "availableModes": ["2560x2880@59.98Hz", "2560x2880@50.00Hz", "1920x1080@60.00Hz"],
}


class Rules(unittest.TestCase):
    def setUp(self):
        self.rules = json.load(open(RULES, encoding="utf-8"))["monitors"]

    def test_the_shipped_layout_matches_the_windows_arrangement(self):
        p = {e["output"]: e for e in disp.plan([ULTRAWIDE, DUALUP], self.rules)}
        self.assertEqual((p["DP-1"]["position"], p["DP-1"]["scale"]), ("0x0", 1))
        self.assertEqual(p["DP-1"]["mode"], "3440x1440@143.97")  # 144 Hz panels advertise 143.97
        # 2560/1.25 = 2048 wide, 2880/1.25 = 2304 tall; placed right of the 3440-wide ultrawide, 783 px up.
        self.assertEqual((p["HDMI-A-1"]["position"], p["HDMI-A-1"]["scale"]), ("3440x-783", 1.25))
        self.assertEqual(p["HDMI-A-1"]["mode"], "2560x2880@59.98")
        self.assertEqual(p["HDMI-A-1"]["note"], "")  # scaled size is a whole number of pixels

    def test_order_of_monitors_does_not_matter(self):
        a = disp.plan([ULTRAWIDE, DUALUP], self.rules)
        b = disp.plan([DUALUP, ULTRAWIDE], self.rules)
        self.assertEqual(sorted(a, key=lambda e: e["output"]), sorted(b, key=lambda e: e["output"]))

    def test_other_ports_get_the_same_layout(self):
        # Same monitors, cables in different ports: only the connector names change.
        u = dict(ULTRAWIDE, name="DP-3")
        d = dict(DUALUP, name="DP-2")
        p = {e["output"]: e for e in disp.plan([u, d], self.rules)}
        self.assertEqual(p["DP-2"]["position"], "3440x-783")

    def test_unknown_monitor_gets_its_best_mode(self):
        other = dict(DUALUP, name="DP-9", description="Dell U2720Q", model="U2720Q", make="Dell",
                     availableModes=["3840x2160@30.00Hz", "3840x2160@60.00Hz", "2560x1440@120.00Hz", "1920x1080@120.00Hz"])
        p = {e["output"]: e for e in disp.plan([ULTRAWIDE, other], self.rules)}
        self.assertEqual((p["DP-9"]["mode"], p["DP-9"]["position"]), ("3840x2160@60", "auto"))
        self.assertIn("no rule matched", p["DP-9"]["note"])

    def test_best_mode_skips_a_slow_top_resolution(self):
        # A 120 Hz monitor whose 4K mode only runs at 30 Hz over this cable: step down to 1440p@120.
        m = {"availableModes": ["3840x2160@30.00Hz", "2560x1440@120.00Hz", "2560x1440@60.00Hz", "1920x1080@120.00Hz"]}
        self.assertEqual(disp.best_mode(m), "2560x1440@120")

    def test_best_mode_when_nothing_reaches_60hz(self):
        m = {"availableModes": ["1920x1080@30.00Hz", "1280x720@50.00Hz"]}
        self.assertEqual(disp.best_mode(m), "1280x720@50")

    def test_best_mode_without_a_mode_list(self):
        self.assertIsNone(disp.best_mode({}))

    def test_missing_primary_means_everything_auto(self):
        p = disp.plan([DUALUP], self.rules)
        self.assertEqual(p[0]["position"], "auto")

    def test_unavailable_mode_falls_back_to_the_best_with_a_note(self):
        low = dict(ULTRAWIDE, availableModes=["1920x1080@60.00Hz", "1920x1080@120.00Hz"])
        p = disp.plan([low, DUALUP], self.rules)
        e = next(x for x in p if x["output"] == "DP-1")
        self.assertEqual(e["mode"], "1920x1080@120")
        self.assertIn("not offered", e["note"])
        self.assertIn("best available", e["note"])

    def test_mode_picking(self):
        m = {"availableModes": ["3440x1440@180.00Hz", "3440x1440@143.97Hz", "3440x1440@59.97Hz"]}
        self.assertEqual(disp.pick_mode(m, "3440x1440@144"), "3440x1440@143.97")
        self.assertEqual(disp.pick_mode(m, "3440x1440@200"), "3440x1440@180")
        self.assertEqual(disp.pick_mode(m, "3440x1440@30"), "3440x1440@59.97")  # nothing at/below 30: the closest above
        self.assertIsNone(disp.pick_mode(m, "1920x1080@60"))
        self.assertIsNone(disp.pick_mode(m, "garbage"))

    def test_disabled_monitors_are_ignored(self):
        off = dict(DUALUP, disabled=True)
        self.assertEqual([e["output"] for e in disp.plan([ULTRAWIDE, off], self.rules)], ["DP-1"])

    def test_fractional_scale_that_does_not_divide_is_flagged(self):
        d = dict(DUALUP)
        rules = [r for r in self.rules]
        rules[1] = dict(rules[1], scale=1.5)  # 2560/1.5 = 1706.67
        e = next(x for x in disp.plan([ULTRAWIDE, d], rules) if x["output"] == "HDMI-A-1")
        self.assertIn("whole number", e["note"])


class Output(unittest.TestCase):
    def test_lua_uses_the_documented_call(self):
        text = disp.lua(disp.plan([ULTRAWIDE, DUALUP], json.load(open(RULES))["monitors"]))
        self.assertIn('hl.monitor({ output = "DP-1", mode = "3440x1440@143.97", position = "0x0", scale = 1 })', text)
        self.assertIn('position = "3440x-783", scale = 1.25 })', text)

    def fake(self, tmp, monitors):
        script = os.path.join(tmp, "hyprctl.py")
        open(script, "w").write("import json,sys\nlog=open(%r,'a'); log.write(' '.join(sys.argv[1:])+'\\n')\nif sys.argv[1]=='monitors': print(json.dumps(%r))\n" % (os.path.join(tmp, "calls.log"), monitors))
        return f"{sys.executable} {script}"

    def run_tool(self, tmp, args, monitors):
        env = dict(os.environ, LABYRINTH_HYPRCTL=self.fake(tmp, monitors), LABYRINTH_DISPLAY_RULES=RULES, XDG_CONFIG_HOME=tmp)
        return subprocess.run([sys.executable, TOOL] + args, env=env, capture_output=True, text=True)

    def test_apply_writes_once_and_reloads_only_on_change(self):
        with tempfile.TemporaryDirectory() as tmp:
            r = self.run_tool(tmp, ["--apply"], [ULTRAWIDE, DUALUP])
            self.assertEqual(r.returncode, 0, r.stderr)
            path = os.path.join(tmp, "hypr", "monitors.lua")
            self.assertTrue(os.path.exists(path))
            calls = open(os.path.join(tmp, "calls.log")).read()
            self.assertIn("reload", calls)
            before = open(os.path.join(tmp, "calls.log")).read().count("reload")
            self.run_tool(tmp, ["--apply"], [ULTRAWIDE, DUALUP])
            self.assertEqual(open(os.path.join(tmp, "calls.log")).read().count("reload"), before)

    def test_print_does_not_write(self):
        with tempfile.TemporaryDirectory() as tmp:
            r = self.run_tool(tmp, ["--print"], [ULTRAWIDE, DUALUP])
            self.assertIn("hl.monitor", r.stdout)
            self.assertFalse(os.path.exists(os.path.join(tmp, "hypr", "monitors.lua")))

    def test_no_hyprland_is_a_clean_failure(self):
        env = dict(os.environ, LABYRINTH_HYPRCTL="definitely-not-a-command")
        r = subprocess.run([sys.executable, TOOL, "--print"], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 1)
        self.assertIn("cannot read monitors", r.stderr)


def kscreen_output(name, modes, current, preferred=None, **kw):
    """One output as `kscreen-doctor -j` reports it (mode ids are strings, ids start at 1)."""
    out = {"name": name, "connected": True, "enabled": True, "currentModeId": str(current), "scale": 1.0,
           "preferredModes": [str(preferred)] if preferred else [],
           "modes": [{"id": str(i + 1), "name": f"{w}x{h}@{r:g}", "refreshRate": r, "size": {"width": w, "height": h}}
                     for i, (w, h, r) in enumerate(modes)]}
    out.update(kw)
    return out


def make_edid(mfg="ACR", name="X34 GS"):
    """A minimal valid 128-byte EDID with a manufacturer id and a monitor-name descriptor."""
    raw = 0
    for ch in mfg:
        raw = (raw << 5) | (ord(ch) - 64)
    b = bytearray(128)
    b[0:8] = b"\x00\xff\xff\xff\xff\xff\xff\x00"
    b[8], b[9] = raw >> 8, raw & 0xFF
    desc = b"\x00\x00\x00\xfc\x00" + (name + "\n").ljust(13).encode("ascii")
    b[54:72] = desc
    return bytes(b)


# What the Acer and the LG DualUp report. The DualUp's native mode is 2560x2880 (16:18), and it also
# lists a 16:9 4K mode, which has more pixels and must not be chosen.
ACER = kscreen_output("DP-1", [(3440, 1440, 59.97), (3440, 1440, 143.97), (2560, 1440, 143.97), (1920, 1080, 60)],
                      current=1, preferred=2)
DUALUP_K = kscreen_output("HDMI-A-1", [(3840, 2160, 60), (3840, 2160, 30), (2560, 2880, 59.98), (2560, 1440, 60), (1920, 1080, 60)],
                          current=1, preferred=3)
MODELS = {"DP-1": "ACR X34 GS", "HDMI-A-1": "GSM LG SDQHD"}


class Kde(unittest.TestCase):
    HDMI = kscreen_output("HDMI-A-1", [(3840, 2160, 29.98), (3840, 2160, 59.94), (2560, 1440, 119.88), (1920, 1080, 119.88)],
                          current=1, preferred=1)

    def test_a_slow_native_mode_steps_down_to_the_best_of_the_same_shape(self):
        args, keys, notes = disp.kde_plan([self.HDMI], {})
        # native 3840x2160 runs at 59.94, which counts as 60 Hz
        self.assertEqual(args, ["output.HDMI-A-1.mode.2"])
        self.assertEqual(len(keys), 1)

    def test_a_30hz_only_native_steps_down_but_keeps_the_shape(self):
        o = kscreen_output("HDMI-A-1", [(3440, 1440, 29.98), (2560, 1080, 59.9), (1920, 1080, 60), (1280, 720, 60)],
                           current=1, preferred=1)
        args, _, _ = disp.kde_plan([o], {})
        self.assertEqual(args, ["output.HDMI-A-1.mode.2"])        # 2560x1080 is 21:9 like the panel; 1920x1080 is not

    def test_the_dualup_does_not_get_the_4k_mode(self):
        args, _, notes = disp.kde_plan([DUALUP_K], {})
        self.assertEqual(args, ["output.HDMI-A-1.mode.3"])        # 2560x2880@59.98, its own
        self.assertIn("2560x2880", notes[0])

    def test_already_best_changes_nothing_but_is_remembered(self):
        out = dict(self.HDMI, currentModeId="2")
        args, keys, _ = disp.kde_plan([out], {})
        self.assertEqual(args, [])
        self.assertEqual(disp.kde_plan([out], {keys[0]: True}), ([], [], []))

    def test_a_monitor_already_handled_is_left_alone(self):
        key = disp.kde_plan([self.HDMI], {})[1][0]
        self.assertEqual(disp.kde_plan([self.HDMI], {key: True}), ([], [], []))   # the user's own choice is never overridden

    def test_a_different_monitor_on_the_same_port_is_handled_again(self):
        key = disp.kde_plan([self.HDMI], {})[1][0]
        other = kscreen_output("HDMI-A-1", [(1920, 1080, 60), (1920, 1080, 144)], current=1, preferred=1)
        self.assertEqual(len(disp.kde_plan([other], {key: True})[1]), 1)

    def test_disconnected_or_disabled_outputs_are_skipped(self):
        off = dict(self.HDMI, enabled=False)
        gone = dict(self.HDMI, connected=False)
        self.assertEqual(disp.kde_plan([off, gone], {}), ([], [], []))

    def test_your_rules_set_mode_scale_and_position(self):
        rules = json.load(open(RULES, encoding="utf-8"))["monitors"]
        args, keys, notes = disp.kde_plan([ACER, DUALUP_K], {}, rules, MODELS)
        self.assertIn("output.DP-1.mode.2", args)                   # 3440x1440@143.97, the 144 Hz mode
        self.assertIn("output.HDMI-A-1.mode.3", args)               # the LG at its own 2560x2880
        self.assertIn("output.HDMI-A-1.scale.1.25", args)
        self.assertIn("output.DP-1.position.0,0", args)
        self.assertIn("output.DP-1.priority.1", args)
        self.assertIn("output.HDMI-A-1.position.3440,-783", args)   # right of the Acer, top 783 px higher
        self.assertEqual(len(keys), 2)

    def test_the_rule_only_applies_to_the_monitor_it_names(self):
        rules = json.load(open(RULES, encoding="utf-8"))["monitors"]
        stranger = kscreen_output("DP-2", [(3840, 2160, 60), (2560, 1440, 144)], current=1, preferred=1)
        args, _, notes = disp.kde_plan([stranger], {}, rules, {"DP-2": "DEL U2720Q"})
        self.assertEqual(args, [])                                  # best mode is already current
        self.assertNotIn("[rule]", notes[0])

    def test_best_mode_keeps_the_native_shape(self):
        m = {"availableModes": ["3840x2160@60.00Hz", "2560x2880@59.98Hz", "1920x1080@60.00Hz"]}
        self.assertEqual(disp.best_mode(m, (2560, 2880)), "2560x2880@59.98")
        self.assertEqual(disp.best_mode(m), "3840x2160@60")        # without a native size it takes the most pixels

    def test_edid_names(self):
        self.assertEqual(disp.edid_model(make_edid("ACR", "X34 GS")), "ACR X34 GS")
        self.assertEqual(disp.edid_model(make_edid("GSM", "LG SDQHD")), "GSM LG SDQHD")
        self.assertEqual(disp.edid_model(b""), "")
        self.assertEqual(disp.edid_model(b"\x01" * 128), "")

    def test_edid_is_read_from_the_connector_directory(self):
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, "card1-DP-1"))
            open(os.path.join(d, "card1-DP-1", "edid"), "wb").write(make_edid("ACR", "X34 GS"))
            self.assertEqual(disp.read_edid_model("DP-1", d), "ACR X34 GS")
            self.assertEqual(disp.read_edid_model("DP-9", d), "")

    @unittest.skipIf(os.name == "nt", "needs a POSIX shell to stand in for kscreen-doctor")
    def test_kde_main_applies_once_and_records(self):
        with tempfile.TemporaryDirectory() as d:
            fake = os.path.join(d, "kscreen-doctor")
            log = os.path.join(d, "calls.txt")
            fixture = os.path.join(d, "out.json")
            json.dump({"outputs": [self.HDMI]}, open(fixture, "w"))
            script = "\n".join([
                "#!/bin/sh",
                'if [ "$1" = -j ]; then cat ' + fixture + '; else echo "$@" >> ' + log + "; fi",
                "",
            ])
            open(fake, "w").write(script)
            os.chmod(fake, 0o755)
            env = dict(os.environ, LABYRINTH_KSCREEN=fake, XDG_CONFIG_HOME=d, LABYRINTH_DRM_ROOT=d)
            r = subprocess.run([sys.executable, TOOL, "--kde"], env=env, capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("output.HDMI-A-1.mode.2", open(log).read())
            r = subprocess.run([sys.executable, TOOL, "--kde"], env=env, capture_output=True, text=True)
            self.assertEqual(open(log).read().count("output.HDMI-A-1.mode.2"), 1)


if __name__ == "__main__":
    unittest.main()
