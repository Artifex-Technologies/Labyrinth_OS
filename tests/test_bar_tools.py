"""Tests for labyrinth-floats and labyrinth-keys against a fake hyprctl. python tests/test_bar_tools.py"""
import importlib.machinery
import importlib.util
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "profile", "airootfs", "usr", "local", "bin")


def load(name):
    loader = importlib.machinery.SourceFileLoader(name, os.path.join(BIN, name))
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


floats = load("labyrinth-floats")
ws = load("labyrinth-workspace")
keys = load("labyrinth-keys")


def fake_hyprctl(tmp, outputs):
    """A stand-in `hyprctl` (a python script) that answers by sub-command."""
    path = os.path.join(tmp, "hyprctl.py")
    with open(path, "w") as f:
        f.write("import json,sys\nout=%r\nprint(json.dumps(out[sys.argv[1]]))\n" % outputs)
    return f"{sys.executable} {path}"


CLIENTS = [
    {"class": "foot", "floating": True, "mapped": True, "workspace": {"id": 2}},
    {"class": "labyrinth-activity", "floating": True, "mapped": True, "workspace": {"id": 2}},
    {"class": "firefox", "floating": False, "mapped": True, "workspace": {"id": 2}},
    {"class": "mpv", "floating": True, "mapped": True, "workspace": {"id": 5}},
    {"class": "ghost", "floating": True, "mapped": False, "workspace": {"id": 2}},
]


class Floats(unittest.TestCase):
    def test_counts_only_mapped_floats_on_this_workspace(self):
        got = floats.floats_on_active_workspace(CLIENTS, 2)
        self.assertEqual(sorted(c["class"] for c in got), ["foot", "labyrinth-activity"])

    def run_main(self, outputs):
        with tempfile.TemporaryDirectory() as tmp:
            env = dict(os.environ, LABYRINTH_HYPRCTL=fake_hyprctl(tmp, outputs))
            r = subprocess.run([sys.executable, os.path.join(BIN, "labyrinth-floats")], env=env, capture_output=True, text=True)
        return json.loads(r.stdout)

    def test_visible_with_count(self):
        out = self.run_main({"activeworkspace": {"id": 2}, "clients": CLIENTS})
        self.assertIn("2", out["text"])
        self.assertIn("foot", out["tooltip"])

    def test_hidden_when_none(self):
        out = self.run_main({"activeworkspace": {"id": 9}, "clients": CLIENTS})
        self.assertEqual(out["text"], "")

    def test_never_crashes_without_hyprland(self):
        env = dict(os.environ, LABYRINTH_HYPRCTL="definitely-not-a-command")
        r = subprocess.run([sys.executable, os.path.join(BIN, "labyrinth-floats")], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0)
        self.assertEqual(json.loads(r.stdout)["text"], "")


BINDS = [
    {"modmask": 64, "key": "RETURN", "has_description": True, "description": "Terminal"},
    {"modmask": 64 | 1, "key": "B", "has_description": True, "description": "Browser"},
    {"modmask": 64 | 4, "key": "v", "has_description": True, "description": "Clipboard history"},
    {"modmask": 0, "key": "XF86AudioMute", "has_description": True, "description": "Mute"},
    {"modmask": 64, "key": "mouse:272", "has_description": False, "description": ""},
    {"modmask": 64, "key": "RETURN", "has_description": True, "description": "Terminal"},
]


class Keys(unittest.TestCase):
    def test_combo_names_modifiers(self):
        self.assertEqual(keys.combo(BINDS[1]), "SUPER + SHIFT + B")
        self.assertEqual(keys.combo(BINDS[2]), "SUPER + CTRL + V")
        self.assertEqual(keys.combo(BINDS[3]), "XF86AudioMute")

    def test_lines_skip_undescribed_and_dedupe_and_sort(self):
        ls = keys.lines(BINDS)
        self.assertEqual(len(ls), 4)
        self.assertTrue(ls[0].endswith("Browser"))
        self.assertFalse(any("mouse" in l for l in ls))

    def test_print_mode_end_to_end(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = dict(os.environ, LABYRINTH_HYPRCTL=fake_hyprctl(tmp, {"binds": BINDS}))
            r = subprocess.run([sys.executable, os.path.join(BIN, "labyrinth-keys"), "--print"], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0)
        self.assertIn("SUPER + RETURN", r.stdout)



STATEFUL_HYPRCTL = """
import json, os, sys
state = os.environ["FAKE_STATE"]; mode = os.environ.get("FAKE_SYNTAX", "classic")
s = json.load(open(state))
a = sys.argv[1:]
if a[:2] == ["workspaces", "-j"]:
    print(json.dumps([{"id": i} for i in s["existing"]]))
elif a[:2] == ["activeworkspace", "-j"]:
    print(json.dumps({"id": s["active"]}))
elif a[:1] == ["dispatch"]:
    n = None
    if mode == "classic" and a[1] == "workspace":
        n = int(a[2])
    elif mode == "lua" and a[1].startswith("hl.dsp.focus"):
        n = int(a[1].split('"')[1])
    if n:
        s["active"] = n
        if n not in s["existing"]:
            s["existing"].append(n)
        json.dump(s, open(state, "w"))
"""


class Workspaces(unittest.TestCase):
    def env(self, tmp, existing, active, syntax="classic"):
        script = os.path.join(tmp, "fake.py")
        with open(script, "w") as f:
            f.write(STATEFUL_HYPRCTL)
        state = os.path.join(tmp, "state.json")
        with open(state, "w") as f:
            json.dump({"existing": existing, "active": active}, f)
        return dict(os.environ, LABYRINTH_HYPRCTL=f"{sys.executable} {script}", FAKE_STATE=state, FAKE_SYNTAX=syntax), state

    def run_cmd(self, env, *args):
        return subprocess.run([sys.executable, os.path.join(BIN, "labyrinth-workspace"), *args], env=env, capture_output=True, text=True)

    def test_lowest_free_number(self):
        self.assertEqual(ws.lowest_free([1, 2, 4]), 3)
        self.assertEqual(ws.lowest_free([]), 1)
        self.assertIsNone(ws.lowest_free(list(range(1, 10))))

    def test_plus_button_shows_below_nine_and_hides_at_nine(self):
        with tempfile.TemporaryDirectory() as tmp:
            env, _ = self.env(tmp, [1], 1)
            out = json.loads(self.run_cmd(env, "status").stdout)
            self.assertEqual(out["text"], "\uf067")
            self.assertIn("1 of 9", out["tooltip"])
            env, _ = self.env(tmp, list(range(1, 10)), 1)
            self.assertEqual(json.loads(self.run_cmd(env, "status").stdout)["text"], "")

    def test_status_without_hyprland_hides_the_button(self):
        env = dict(os.environ, LABYRINTH_HYPRCTL="definitely-not-a-command")
        self.assertEqual(json.loads(self.run_cmd(env, "status").stdout)["text"], "")

    def test_new_goes_to_the_lowest_unused_desktop(self):
        with tempfile.TemporaryDirectory() as tmp:
            env, state = self.env(tmp, [1, 2, 4], 1)
            self.assertEqual(self.run_cmd(env, "new").returncode, 0)
            self.assertEqual(json.load(open(state))["active"], 3)

    def test_works_with_the_lua_style_dispatch_too(self):
        with tempfile.TemporaryDirectory() as tmp:
            env, state = self.env(tmp, [1], 1, syntax="lua")
            self.assertEqual(self.run_cmd(env, "new").returncode, 0)
            self.assertEqual(json.load(open(state))["active"], 2)

    def test_reports_failure_when_neither_syntax_works(self):
        with tempfile.TemporaryDirectory() as tmp:
            env, state = self.env(tmp, [1], 1, syntax="none")
            self.assertEqual(self.run_cmd(env, "new").returncode, 1)
            self.assertEqual(json.load(open(state))["active"], 1)

    def test_goto_rejects_out_of_range(self):
        with tempfile.TemporaryDirectory() as tmp:
            env, state = self.env(tmp, [1], 1)
            self.assertEqual(self.run_cmd(env, "goto", "10").returncode, 1)
            self.assertEqual(self.run_cmd(env, "goto", "5").returncode, 0)
            self.assertEqual(json.load(open(state))["active"], 5)


if __name__ == "__main__":
    unittest.main()
