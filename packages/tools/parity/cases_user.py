"""The cases of the shell scripts' ports that run as the user (loaded by compare.py): labyrinth-security-setting and, below, the
rest. They run in cases_root.py's world (a root folder of their own in a user and mount namespace; the home is /home/tester
there), so that the system files they read (/usr/share/labyrinth, /etc/pacman.conf, /proc ...) are the world's, and every
program they start is a stand-in that writes down its call. Nothing of this computer's home, settings or services is touched."""
import os

import __main__ as P
import cases_root as C

W = C.W
HOME = "/home/tester"
UENV = {"HOME": HOME, "USER": "tester", "XDG_RUNTIME_DIR": "/run/user/1001"}


def user_case(prog, name, steps, files=None, stubs=(), fakes=(), env=None, differs=(), norm_more=None, extra=None):
    f = {HOME + "/": ("dir", 0o755), "/run/user/1001/": ("dir", 0o700)}
    f.update(files or {})
    setup = C.world(files=f, stubs=stubs, fakes=list(fakes), passwd="tester:x:1001:1001::/home/tester:/bin/bash\n", group="tester:x:1001:\n",
                    extra=extra)
    C.case(prog, name, steps, setup, env=dict(UENV, **(env or {})), differs=differs, norm_more=norm_more)


# ---------------------------------------------------------------- labyrinth-security-setting

SS = "labyrinth-security-setting"


def sec(name, steps, files=None, env=None, differs=(), fakes=()):
    user_case(SS, name, steps, files=files, stubs=("heimdalld",), env=env, differs=differs, fakes=fakes)


sec("usage", [{"args": []}, {"args": ["bogus"]}, {"args": ["scan"]}, {"args": ["scan", ""]}, {"args": ["auto-quarantine"]},
              {"args": ["auto-quarantine", "maybe"]}])
sec("scan a folder", [{"args": ["scan", HOME + "/Downloads"]}, {"args": ["scan", "rel/dir", "extra"]}])
sec("scan a folder named like an option (fixed: ./-x)", [{"args": ["scan", "--quarantine"]}, {"args": ["scan", "-"]}])
sec("scan: heimdalld fails", [{"args": ["scan", "/x"]}], fakes=[("heimdalld", {"rc": 3, "out": "found 1\n", "err": "bad\n"})])
sec("scan: XDG_STATE_HOME", [{"args": ["scan", "/x"]}], env={"XDG_STATE_HOME": HOME + "/st"})
sec("auto-quarantine: on, status, off, status", [{"args": ["auto-quarantine", "status"]}, {"args": ["auto-quarantine", "off"]},
                                                 {"args": ["auto-quarantine", "status"]}, {"args": ["auto-quarantine", "on"]},
                                                 {"args": ["auto-quarantine", "status"]}])
sec("auto-quarantine status: written by hand", [{"args": ["auto-quarantine", "status"]}],
    files={HOME + "/.config/labyrinth/scan.conf": "# action = notify\n  action\t=  notifyx\n"})
sec("auto-quarantine: XDG_CONFIG_HOME", [{"args": ["auto-quarantine", "off"]}], env={"XDG_CONFIG_HOME": HOME + "/cfg"})
sec("auto-quarantine: the folder cannot be made", [{"args": ["auto-quarantine", "on"]}], files={HOME + "/.config": "a file\n"},
    differs=(("err", 0),))
sec("HOME unset", [{"args": ["auto-quarantine", "status"]}], env={"HOME": None}, differs=(("err", 0),))
