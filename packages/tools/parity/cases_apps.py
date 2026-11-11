"""The cases of labyrinth-apps (loaded by compare.py): the repository's components.json and made-up ones; pacman, yay, makepkg,
sudo, usermod and systemctl are stand-ins (PATH has no system folders, so nothing real is ever installed)."""
import json
import os

import __main__ as P

W = P.WORLD
REAL = os.path.join(P.REPO, "profile", "airootfs", "usr", "share", "labyrinth", "components.json")
MAN = os.path.join(W, "components.json")
PB = os.path.join(W, "pkgbuilds")
PACMAN = r"""#!/bin/sh
line=pacman
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
case "$1" in
  -Qq) printf '%s\n' $INSTALLED; exit ${RC_Q:-0} ;;
  -Si) case " $IN_REPOS " in *" $2 "*) exit 0 ;; *) exit 1 ;; esac ;;
  *) exit ${RC_pacman:-0} ;;
esac
"""
MAKEPKG = r"""#!/bin/sh
line="makepkg (in $(basename "$(pwd)"), files: $(ls | tr '\n' ' '))"
for a in "$@"; do case "$a" in */makepkg.conf) line="$line [<TMP>/makepkg.conf]"; cat "$a" >> "$WORLD/calls" ;; *) line="$line [$a]" ;; esac; done
printf '%s\n' "$line" >> "$WORLD/calls"
exit ${RC_makepkg:-0}
"""
MADE = {"components": [
    {"id": "a", "name": "App A", "description": "d", "packages": ["pa", "pb"], "notes": "log out"},
    {"id": "b", "name": "B", "description": "d", "aur": ["b-bin"], "proprietary": True, "add_groups": ["plugdev", "input"]},
    {"id": "c", "name": "C", "description": "d", "pkgbuild": "c-git", "enable_global": ["c.timer"], "enable_system": ["c.service"],
     "after_install": [["c-setup", "--x"], "notalist", [], ["ok", 1]]},
    {"id": "long-name-component", "name": "L", "description": "d", "packages": ["l"], "unverified_packages": ["l"]}],
    "presets": {"none": [], "ab": ["a", "b"]}}


def world(manifest=MADE, pkgbuilds=True):
    def go(paths):
        os.makedirs(os.path.join(W, "pybin"), exist_ok=True)
        for t in ("python3", "basename", "tr", "cat", "ls"):
            os.symlink("/usr/bin/" + t, os.path.join(W, "pybin", t))
        P.stubs(["yay", "sudo", "usermod", "systemctl", "agora-sudo"])
        P.write(os.path.join(W, "bin", "pacman"), PACMAN, 0o755)
        P.write(os.path.join(W, "bin", "makepkg"), MAKEPKG, 0o755)
        P.write(MAN, manifest if isinstance(manifest, (str, bytes)) else json.dumps(manifest))
        if pkgbuilds:
            P.write(os.path.join(PB, "c-git", "PKGBUILD"), "pkgname=c-git\n")
            P.write(os.path.join(PB, "c-git", "sub", "x.patch"), "x\n")
        os.makedirs(os.path.join(W, "sync"), exist_ok=True)
        P.write(os.path.join(W, "sync", "core.db"), "")
        os.makedirs(os.path.join(W, "tmp"), exist_ok=True)
    return go


ENV = {"PATH": os.path.join(W, "bin") + ":" + os.path.join(W, "pybin"), "LABYRINTH_COMPONENTS": MAN, "LABYRINTH_PKGBUILDS": PB,
       "LABYRINTH_PACMAN_SYNC": os.path.join(W, "sync"), "TMPDIR": os.path.join(W, "tmp"), "INSTALLED": "pa l"}


def case(name, steps, manifest=MADE, env=None, **k):
    e = dict(ENV)
    e.update(env or {})
    P.case("labyrinth-apps", name, steps, setup=world(manifest), env=e, **k)


for args in [[], ["-h"], ["--help"], ["nope"], ["info"], ["info", "a", "b"], ["--dry-run"]]:
    case("arguments: %r" % args, [{"args": args}])
case("list and info", [{"args": ["list"]}] + [{"args": ["info", i]} for i in ("a", "b", "c", "long-name-component", "zzz")])
case("the repository's components.json: list, info, dry runs", [{"args": ["list"]}, {"args": ["info", "steam"]}, {"args": ["install", "--preset", "everything", "--dry-run"]},
                                                                   {"args": ["remove", "--preset", "everything", "--dry-run"]}],
     env={"LABYRINTH_COMPONENTS": REAL, "INSTALLED": "obsidian rclone limbo-git"})
case("install everything, for real (stand-ins), the build in a copy", [{"args": ["install", "a", "b", "c", "a"]}], env={"USER": "tester"})
case("install from Agora: its password box", [{"args": ["install", "a", "b", "c"]}], env={"AGORA_SUDO": os.path.join(W, "bin", "agora-sudo"), "SUDO_USER": "ada"})
case("install from Agora under the old name", [{"args": ["install", "c", "--dry-run"]}], env={"LABYRINTH_SUDO": "/x/my sudo"})
case("install: c is in a repository", [{"args": ["install", "c"]}], env={"IN_REPOS": "c-git"})
case("install on a system never synced", [{"args": ["install", "a", "c"]}, {"args": ["install", "a", "--dry-run"], "env": {"LABYRINTH_PACMAN_SYNC": ""}}],
     env={"LABYRINTH_PACMAN_SYNC": os.path.join(W, "nosync")})
case("install: a step fails, the rest is not run", [{"args": ["install", "a", "c"]}], env={"RC_pacman": "1"})
case("install: the build fails", [{"args": ["install", "c"]}], env={"RC_makepkg": "4"})
case("install: no PKGBUILD folder", [{"args": ["install", "c"]}], env={"LABYRINTH_PKGBUILDS": os.path.join(W, "none")})
case("install: nothing to do", [{"args": ["install", "long-name-component"]}])
case("presets and their errors", [{"args": ["install", "--preset", "ab", "--dry-run"]}, {"args": ["install", "--preset", "none", "--dry-run"]},
                                  {"args": ["install", "--preset"]}, {"args": ["install", "--preset", "zz"]}, {"args": ["install", "x", "--preset", "ab", "c", "--dry-run"]},
                                  {"args": ["install", "nope"]}, {"args": ["remove"]}])
case("remove: what is installed, units first", [{"args": ["remove", "a", "b", "c"]}], env={"INSTALLED": "pa b-bin c-git"})
case("remove: nothing installed", [{"args": ["remove", "a"]}], env={"INSTALLED": ""})
case("pacman is missing", [{"args": ["list"]}, {"args": ["install", "a"]}, {"args": ["install", "a", "--dry-run"]}], env={"PATH": os.path.join(W, "pybin")})
case("pacman -Qq fails: nothing counts as installed", [{"args": ["list"]}], env={"RC_Q": "1"})
for label, m in [("missing", None), ("not JSON", "{"), ("not UTF-8", b"\xff"), ("no components", {"x": 1}), ("a component without id", {"components": [{"name": "x"}]}),
                 ("a preset naming nothing", {"components": [{"id": "a"}], "presets": {"p": ["a", "zz"]}}), ("a list", "[1]"),
                 ("no components at all", {"components": []}), ("a component without a name", {"components": [{"id": "a", "packages": ["x"]}]})]:
    case("components.json: " + label, [{"args": ["list"]}, {"args": ["install", "a", "--dry-run"]}],
         manifest=m if m is not None else MADE, env={"LABYRINTH_COMPONENTS": os.path.join(W, "missing.json")} if m is None else None,
         differs={("err", 0), ("err", 1)} if label == "not JSON" else ())  # the JSON parser's own words differ (README)
