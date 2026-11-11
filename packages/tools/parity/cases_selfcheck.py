"""The cases of labyrinth-selfcheck (loaded by compare.py). It reads fixed places of the system, so each run is in a world of its
own (compare.NS: a user and mount namespace): pacman's log, /usr/lib/modules, /etc/systemd/system, /etc/greetd, /etc/heimdall
and /proc are the world's folders seen there, /usr/bin has the world's additions and removals laid over it; pgrep, hyprctl,
busctl, systemctl, curl and pacman are stand-ins. Only the running kernel's version is the computer's own."""
import json
import os
from datetime import datetime
from zoneinfo import ZoneInfo

import __main__ as P

W = P.WORLD
FS = os.path.join(W, "fs")
REL = os.uname().release
FLAVOUR = REL.split("-")[-1]
TZ = ZoneInfo("America/Denver")  # compare.py's TZ


def epoch(stamp):
    return datetime.strptime(stamp, "%Y-%m-%dT%H:%M:%S").replace(tzinfo=TZ).timestamp()


LOG = """[2026-10-14T10:00:00-0600] [ALPM] installed hyprland (0.56.1-1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland (0.56.1-1 -> 0.56.2-1)
[2026-10-16T06:32:12-0600] [ALPM] upgraded yggdrasil-lock (0.1.0-1 -> 0.1.0-2)
[2026-10-16T07:00:00-0600] [ALPM] upgraded hyprland-extra (1 -> 2)
[2026-10-17T08:00:00-0600] [ALPM] reinstalled yggdrasil-panel (0.2-1)
"""
UPGRADED = epoch("2026-10-17T08:00:00")
GREETD = 'vt = 7\n[default_session]\ncommand = "cage -s -- /usr/bin/yggdrasil-greeter"\nuser = "greeter"\n'
MONITORS = json.dumps([{"name": "DP-1", "width": 3440, "height": 1440, "disabled": False},
                       {"name": "HDMI-A-1", "width": 1920, "height": 1080, "disabled": True}])
BUS = ("org.freedesktop.DBus 1 dbus-broker user - - -\n"
       "org.freedesktop.Notifications 2345 yggdrasil-panel tester :1.20 user@1000.service - -\n")
HEIMDALL = {"fim_paths": ["/usr/bin/solidcraft", "/usr/bin/sudo"]}

SYSTEMCTL = r"""#!/bin/sh
line=systemctl
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
case "$1" in
  is-active) u=${2%.service}; eval "st=\${ACTIVE_$(printf '%s' "$u" | tr -c 'A-Za-z0-9' '_'):-inactive}"; printf '%s\n' "$st"
             [ "$st" = active ] || exit 3 ;;
  cat) case " $UNITS " in *" ${2%.service} "*) printf '[Unit]\n' ;; *) echo "No files found for $2." >&2; exit 1 ;; esac ;;
  --failed) printf '%b' "${FAILED:-}" ;;
esac
"""
PGREP = r"""#!/bin/sh
line=pgrep
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
eval "pid=\${PID_$(printf '%s' "$2" | tr -c 'A-Za-z0-9' '_'):-}"
[ -n "$pid" ] || exit 1
printf '%b' "$pid"
"""
GOOD_ENV = {"PID_Hyprland": "4242\\n", "PID_yggdrasil_panel": "4300\\n", "OUT_hyprctl": MONITORS, "OUT_busctl": BUS, "OUT_curl": "ok",
            "ACTIVE_ollama": "active", "ACTIVE_limbo": "active", "ACTIVE_heimdall": "active", "ACTIVE_sarina_service": "active",
            "ACTIVE_yggdrasil_powerlimit": "active", "UNITS": "ollama limbo heimdall sarina-service yggdrasil-powerlimit",
            "OUT_pacman": "solidcraft-bin\nyay\n"}


def world(log=LOG, mods=(REL,), dm="/usr/lib/systemd/system/greetd.service", greetd=GREETD, heimdall=HEIMDALL, started=UPGRADED + 600,
          greeter=True, usr_bin_extra=("solidcraft",), pids=(4242, 4300), no_log=False):
    def go(paths):
        os.makedirs(os.path.join(W, "pybin"), exist_ok=True)
        for t in ("python3", "basename", "tr", "cat"):
            os.symlink("/usr/bin/" + t, os.path.join(W, "pybin", t))
        P.stubs(["hyprctl", "busctl", "curl", "pacman"])
        P.write(os.path.join(W, "bin", "systemctl"), SYSTEMCTL, 0o755)
        P.write(os.path.join(W, "bin", "pgrep"), PGREP, 0o755)
        mounts = [("hostname", "parity-host")]
        os.makedirs(os.path.join(FS, "log"))
        if not no_log:
            P.write(os.path.join(FS, "log", "pacman.log"), log)
        mounts.append(("bind", os.path.join(FS, "log"), "/var/log"))
        os.makedirs(os.path.join(FS, "modules"))
        for m in mods:
            os.makedirs(os.path.join(FS, "modules", m))
        mounts.append(("bind", os.path.join(FS, "modules"), "/usr/lib/modules"))
        os.makedirs(os.path.join(FS, "systemd"))
        if dm:
            os.symlink(dm, os.path.join(FS, "systemd", "display-manager.service"))
        mounts.append(("bind", os.path.join(FS, "systemd"), "/etc/systemd/system"))
        os.makedirs(os.path.join(FS, "greetd"))
        if greetd is not None:
            P.write(os.path.join(FS, "greetd", "config.toml"), greetd)
        mounts.append(("bind", os.path.join(FS, "greetd"), "/etc/greetd"))
        os.makedirs(os.path.join(FS, "heimdall"))
        if heimdall is not None:
            P.write(os.path.join(FS, "heimdall", "config.json"), heimdall if isinstance(heimdall, (str, bytes)) else json.dumps(heimdall))
        mounts.append(("bind", os.path.join(FS, "heimdall"), "/etc/heimdall"))
        for pid in pids:
            d = os.path.join(FS, "proc", str(pid))
            os.makedirs(d)
            os.utime(d, (started, started))
        os.makedirs(os.path.join(FS, "proc"), exist_ok=True)
        mounts.append(("bind", os.path.join(FS, "proc"), "/proc"))
        for name in usr_bin_extra:
            P.write(os.path.join(W, "ov", "usr", "bin", "up", name), "#!/bin/sh\n", 0o755)
        if not greeter:
            mounts.append(("whiteout", "/usr/bin", "yggdrasil-greeter"))
        mounts.append(("overlay", "/usr/bin"))
        P.world_of_its_own(mounts)
    return go


ENV = {"PATH": os.path.join(W, "bin") + ":" + os.path.join(W, "pybin")}


def case(name, steps=None, env=None, quiet_too=True, **world_args):
    e = dict(ENV)
    e.update(GOOD_ENV)
    e.update(env or {})
    steps = steps or ([{"args": []}, {"args": ["--quiet"]}] if quiet_too else [{"args": []}])
    for s in steps:
        s["wrap"] = P.NS_WRAP
    P.case("labyrinth-selfcheck", name, steps, setup=world(**world_args), env=e)


case("everything fine", steps=[{"args": []}, {"args": ["--quiet"]}, {"args": ["--quiet", "x"]}, {"args": ["-h"]}])
# the session against its packages
case("the session started before three of its packages were upgraded", started=epoch("2026-10-15T00:00:00"))
case("started before even hyprland was installed", started=epoch("2026-10-01T00:00:00"))
case("started 30 s before the last upgrade: within the slack", started=UPGRADED - 30)
case("started 61 s before the last upgrade", started=UPGRADED - 61)
case("Hyprland is not running, no screens: a warning", env={"PID_Hyprland": "", "OUT_hyprctl": "[]"})
case("Hyprland is not running, a FAIL after warnings: exit 1 (fixed)", env={"PID_Hyprland": "", "PID_yggdrasil_panel": ""})
case("pgrep names a process without /proc", env={"PID_Hyprland": "777\\n4242\\n"})
case("pgrep says more, the first counts", env={"PID_Hyprland": "  4300 4242\\n"}, started=epoch("2026-10-01T00:00:00"))
case("the session's /proc time is 0: no screens is only a warning", started=0, env={"OUT_hyprctl": "[]"})
# pacman's log
case("no pacman log", no_log=True)
case("a log with bytes that are not UTF-8 and odd lines", log=b"\xff\xfe junk\n[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland (1)\n"
     b"[2026-10-18T06:32:12+0100] [ALPM] installed yggdrasil-session \xe9\n[bad] [ALPM] upgraded hyprland (2)\n", started=epoch("2026-10-17T00:00:00"))
case("a date strptime refuses stops the script (and the program)", log="[2026-02-30T10:00:00-0600] [ALPM] installed hyprland (1)\n")
case("a month 13", log="[2026-13-01T10:00:00-0600] [ALPM] installed hyprland (1)\n")
case("second 61, Feb 29 of a leap year, digits of other scripts", log="[2028-02-29T23:59:61-0600] [ALPM] installed hyprland (1)\n"
     "[2026-01-1٢T0٥:00:00-0600] [ALPM] upgraded yggdrasil-panel (1)\n[٢٠٢٦-01-12T05:00:00-0600] [ALPM] upgraded yggdrasil-lock (1)\n",
     started=epoch("2026-01-01T00:00:00"))
case("log lines that almost match", log="[2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland\n[2026-10-16T06:32:12-0600]  [ALPM] upgraded hyprland (1)\n"
     "[2026-10-16T06:32:12Z0600] [ALPM] upgraded hyprland (1)\n[2026-10-16 06:32:12-0600] [ALPM] upgraded hyprland (1)\n"
     "[2026-10-16T06:32:12-0600] [ALPM] removed hyprland (1)\n [2026-10-16T06:32:12-0600] [ALPM] upgraded hyprland (1)\n"
     "[2026-10-16T06:32:12-060] [ALPM] upgraded hyprland (1)\n[2026-10-17T06:32:12-0600] [ALPM] upgraded hyprland", started=epoch("2026-10-01T00:00:00"))
case("year 0", log="[0000-01-01T00:00:00-0600] [ALPM] installed hyprland (1)\n")
# kernels
case("the running kernel is not installed any more", mods=("1.0.0-1-" + FLAVOUR, "0.9-1-" + FLAVOUR, "6.1-lts"))
case("a newer kernel is installed (by version: 10 after 9, fixed)", mods=(REL, "999.10.0-1-" + FLAVOUR, "999.9.0-1-" + FLAVOUR))
case("an older one left over", mods=(REL, "0.9.0-1-" + FLAVOUR))
case("no kernel of this flavour", mods=("6.1-lts",))
case("no kernels at all", mods=())
# screens and the panel
for label, out in [("not JSON", "garbage"), ("empty", ""), ("a number", "5"), ("null", "null"), ("an empty object", "{}"), ("an empty text", '""'),
                   ("all disabled", '[{"name": "a", "disabled": 1}]'), ("odd values", '[{"name": null, "width": 1.5, "height": true}, {"disabled": 0, "name": ["x"], "width": {"a": 1}}]'),
                   ("an object: the script stopped", '{"a": 1}'), ("a list of numbers: the script stopped", "[1]"), ("a text: the script stopped", '"x"')]:
    case("hyprctl answers " + label, env={"OUT_hyprctl": out}, quiet_too=False)
case("the panel is not running", env={"PID_yggdrasil_panel": ""})
# the login screen
case("no display manager", dm=None)
case("the display manager is sddm", dm="/usr/lib/systemd/system/sddm.service")
case("display-manager.service is a link to a link", dm="/etc/systemd/system/other.service")
case("greetd without a greeter command", greetd="vt = 7\n[default_session]\nuser = \"greeter\"\n")
case("greetd's command over lines and spaces", greetd="\n  \n command\t=  \"x\"  \r\n", quiet_too=False)
for i, conf in enumerate(['command\n=\n"x"\n', 'command = "x" # c\n', 'command = "a"b"\n', 'command = "\n"\n', '  xcommand = "x"\n', 'command = ""',
                          'command = "x"\r\n', '\x85command = "x"', 'commands = "x"\n', 'command="x"\u2028', 'command = "x" \x0c y\n', '# command = "x"\n',
                          '[s]\ncommand =\t"cage -s -- /usr/bin/yggdrasil-greeter"\t\n', 'command = x"\n', 'command "x"\n']):
    case("greetd's command, written oddly (%d)" % i, greetd=conf, quiet_too=False)
case("no greetd config", greetd=None, quiet_too=False)
case("a greetd config that is not UTF-8: the script stopped", greetd=b"command = \"\xff\"\n", quiet_too=False)
case("the greeter is not installed", greeter=False)
# notifications
case("mako serves notifications", env={"OUT_busctl": "org.freedesktop.Notifications 99 mako tester :1.3 user@1000.service - -\n"})
case("nobody serves notifications", env={"OUT_busctl": ""}, quiet_too=False)
case("a short busctl line: the script stopped", env={"OUT_busctl": "org.freedesktop.Notifications 99\n"}, quiet_too=False)
# services
case("services in every state", env={"ACTIVE_ollama": "failed", "ACTIVE_limbo": "", "ACTIVE_heimdall": "  activating \n", "ACTIVE_sarina_service": "inactive",
                                     "UNITS": "ollama limbo heimdall yggdrasil-powerlimit", "FAILED": "a.service loaded failed failed A\\n  \\nb.mount loaded failed failed B\\n"})
case("no service installed", env={"ACTIVE_ollama": "", "ACTIVE_limbo": "", "ACTIVE_heimdall": "", "ACTIVE_sarina_service": "", "ACTIVE_yggdrasil_powerlimit": "", "UNITS": ""})
# heimdall
case("Heimdall does not answer", env={"OUT_curl": "", "RC_curl": "7"})
case("an AUR app not in the file-integrity list", heimdall={"fim_paths": ["/usr/bin/sudo"]}, usr_bin_extra=("solidcraft", "papercraft"),
     env={"OUT_pacman": "solidcraft-bin\npapercraft-bin\nghostcraft-bin\ncraft-bin\nx\n"})
case("fim_paths as text, an object, numbers", heimdall={"fim_paths": "/usr/bin/solidcraft"})
case("fim_paths an object", heimdall={"fim_paths": {"/usr/bin/solidcraft": 1}})
case("fim_paths with numbers and null", heimdall={"fim_paths": [1, None, 2.5, True, "/usr/bin/solidcraft"]})
case("no fim_paths", heimdall={"x": 1}, quiet_too=False)
case("Heimdall's config is not JSON", heimdall="{", quiet_too=False)
case("Heimdall's config is missing", heimdall=None, quiet_too=False)
case("fim_paths null: the script stopped", heimdall={"fim_paths": None}, quiet_too=False)
case("fim_paths with a list in it: the script stopped", heimdall={"fim_paths": [["x"]]}, quiet_too=False)
case("Heimdall's config a list: the script stopped", heimdall=[1], quiet_too=False)
# the tools themselves missing
case("no tools at all", env={"PATH": os.path.join(W, "pybin")})
