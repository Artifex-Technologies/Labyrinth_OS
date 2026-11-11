"""The cases of steam_playall and labyrinth-first-components (loaded by compare.py).

steam_playall edits a made-up config.vdf in the world; Steam is never started or looked at. labyrinth-first-components runs with
parity/fastclock.c preloaded (its 15 s and 10 s waits pass at once, on its own clocks), a stand-in nmcli that answers what the
case says (and how often), stand-ins for notify-send and labyrinth-apps, and /usr/lib/agora seen as a folder of the world."""
import json
import os

import __main__ as P

W = P.WORLD
VDF = os.path.join(W, "config.vdf")

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


def vdf(text, mode=0o644, link=False):
    def go(paths):
        if text is not None:
            P.write(VDF + (".real" if link else ""), text, mode)
        if link:
            os.symlink("config.vdf.real", VDF)
    return go


def playall(name, text, args=("{WORLD}/config.vdf",), steps=None, **k):
    P.case("steam_playall.py", name, steps or [{"args": list(args)}, {"args": list(args)}], setup=vdf(text, **k))


playall("arguments: none", None, steps=[{"args": []}])
playall("arguments: four", None, steps=[{"args": ["a", "b", "c"]}])
playall("an empty block: added, then already", BASE % "")
playall("beside a game's own tool", BASE % GAME)
playall("a default already chosen: never changed", BASE % ZERO)
playall("a \"0\" nested in a game's entry does not count", BASE % '\t\t\t\t\t"620"\n\t\t\t\t\t{\n\t\t\t\t\t\t"0"\n\t\t\t\t\t\t{\n\t\t\t\t\t\t}\n\t\t\t\t\t}\n')
playall("another tool by name", BASE % "", args=("{WORLD}/config.vdf", "proton-cachyos-10.0-20261101"))
playall("a tool name with odd characters", BASE % "", args=("{WORLD}/config.vdf", 'a"b\tc ü'))
playall("\\r\\n line ends are kept (fixed)", (BASE % GAME).replace("\n", "\r\n"))
playall("mixed line ends and a lone \\r", (BASE % "").replace("\t\t\t\t}\n\t\t\t}", "\t\t\t\t}\r\t\t\t}").replace('"Steam"\n', '"Steam"\r\n'))
for i, text in enumerate(['"CompatToolMapping"\r\r\n{\n}\n', '"CompatToolMapping" \t\n\t {\t \r\n}\n', 'x\n"CompatToolMapping"\n{\n"0"\n}\n', 'x"CompatToolMapping"\n{\n}\n',
                          '"CompatToolMapping"\n\n{\n}\n', '"CompatToolMapping"\n{ x\n}\n', '"CompatToolMapping"\n{', '"CompatToolMapping"\n{\n',
                          '\t"CompatToolMapping"\n{\r\n\t\t"0"\r\n}\n', 'a\n\t "CompatToolMapping"\n\t{\n\t}\n"CompatToolMapping"\n{\n}\n']):
    playall("the block, written oddly (%d)" % i, text)
playall("no CompatToolMapping block", '"InstallConfigStore"\n{\n}\n')
playall("a block on one line is not taken", '"CompatToolMapping" { }\n')
playall("unbalanced braces", '"CompatToolMapping"\n{\n\t"620"\n\t{\n')
playall("indented with spaces, braces in a name", '  "CompatToolMapping"  \n   {  \n\t"1" { "name" "a}{" }\n   }\n')
playall("odd whitespace around \"0\"", BASE % '\t\t\t\t\t  "0" \x0b\n\t\t\t\t\t{\n\t\t\t\t\t}\n')
playall("text after the block, a second block", (BASE % "") + '"CompatToolMapping"\n{\n"0"\n}\n')
playall("the file's mode is kept, and the backup's", BASE % "", mode=0o600)
playall("through a link", BASE % "", link=True)
playall("no such file: the script stopped", None)
playall("a folder: the script stopped", None, args=("{WORLD}",))
playall("not UTF-8: the script stopped", b"\xff\xfe" + (BASE % "").encode())
playall("empty file", "")

# ---- labyrinth-first-components

LIST = os.path.join(W, "first.json")
NMCLI = r"""#!/bin/sh
line=nmcli
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
n=$(cat "$WORLD/nm-count" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > "$WORLD/nm-count"
if [ "$n" -ge "${FULL_AFTER:-1}" ]; then echo full; else echo limited; fi
"""
APPS = r"""#!/bin/sh
line=labyrinth-apps
for a in "$@"; do line="$line [$a]"; done
printf '%s AGORA_SUDO=%s\n' "$line" "${AGORA_SUDO:-unset}" >> "$WORLD/calls"
echo "==> installing $*"
echo "a warning" >&2
exit ${RC_apps:-0}
"""


def first_world(data, agora=False, apps=True, notify=True):
    def go(paths):
        os.makedirs(os.path.join(W, "pybin"), exist_ok=True)
        for t in ("basename", "tr", "cat"):
            os.symlink("/usr/bin/" + t, os.path.join(W, "pybin", t))
        if notify:
            P.stubs(["notify-send"])
        P.write(os.path.join(W, "bin", "nmcli"), NMCLI, 0o755)
        if apps:
            P.write(os.path.join(W, "bin", "labyrinth-apps"), APPS, 0o755)
        if data is not None:
            P.write(LIST, data if isinstance(data, (str, bytes)) else json.dumps(data))
        os.makedirs(os.path.join(W, "agora"))
        if agora:
            P.write(os.path.join(W, "agora", "sudo"), "#!/bin/sh\n", 0o755)
        P.world_of_its_own([("bind", os.path.join(W, "agora"), "/usr/lib/agora")])
    return go


def first(name, data, env=None, steps=None, **k):
    e = {"PATH": os.path.join(W, "bin") + ":" + os.path.join(W, "pybin"), "LABYRINTH_FIRST_COMPONENTS": LIST, "USER": "ada",
         "LD_PRELOAD": P.fastclock()}
    e.update(env or {})
    steps = steps or [{"args": []}]
    for s in steps:
        s["wrap"] = P.NS_WRAP
    P.case("labyrinth-first-components", name, steps, setup=first_world(data, **k), env=e)


ADA = {"user": "ada", "components": ["sarina", "heimdall"]}
first("no list", None)
first("the list is someone else's", {"user": "bob", "components": ["sarina"]})
first("no user in the list, none in the environment", {"components": ["sarina"]}, env={"USER": None})
first("no user in the list, one in the environment", {"components": ["sarina"]})
first("USER empty, the list's user empty", {"user": "", "components": ["sarina"]}, env={"USER": ""})
first("the user is a number", {"user": 5, "components": ["sarina"]})
first("not JSON", "{")
first("not UTF-8", b"\xff")
first("nothing to install", {"user": "ada", "components": []})
first("only bad ids", {"user": "ada", "components": ["bad; rm -rf /", "", "-", "x_y", 5, None, ["a"]]})
first("ids of other scripts and dashes", {"user": "ada", "components": ["a-b", "été", "٣x", "--"]})
first("online at once: installed, the list removed", ADA, steps=[{"args": []}, {"args": ["--ignored"]}])
first("online after three tries", ADA, env={"FULL_AFTER": "3"})
first("never online: 13 tries over 120 s, the list kept", ADA, env={"FULL_AFTER": "99"})
first("the install fails: the list kept, said so", ADA, env={"RC_apps": "3"})
first("Agora's password box is there", ADA, agora=True)
first("components as text: its letters", {"user": "ada", "components": "ab"})
first("components as an object: its keys", {"user": "ada", "components": {"sarina": 1, "x y": 2}})
first("components null: the script stopped", {"user": "ada", "components": None})
first("components a number: the script stopped", {"user": "ada", "components": 5})
first("the list is a list: the script stopped", ["sarina"])
first("labyrinth-apps missing: the script stopped", ADA, apps=False)
first("notify-send missing: the script stopped", ADA, notify=False)
first("the log folder cannot be made: the script stopped", ADA, env={"HOME": "{WORLD}/first.json"})
