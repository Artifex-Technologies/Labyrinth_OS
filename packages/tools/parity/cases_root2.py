"""The cases of labyrinth-agent-setup, labyrinth-agent-window, labyrinth-ecosystem-setup, labyrinth-hardware-setup and
labyrinth-grub-theme (loaded by compare.py), on cases_root.py's world: a root folder of their own in a user and mount namespace,
stand-ins on PATH that write down every call. sudo, visudo, useradd and the rest are stand-ins: no rule, account or service of
this computer is touched."""
import os
import re

import __main__ as P
import cases_root as C

W = C.W


def norm(t):
    t = re.sub(r"/tmp/tmp\.[A-Za-z0-9]{10}", "<SUDOERS-TMP>", t)
    t = re.sub(r"/etc/sudoers\.d/\.5[01]-labyrinth-agent(-window)?\.[A-Za-z0-9]{6}", "<SUDOERS-TMP>", t)
    t = re.sub(r"SARINA_SERVICE_TOKEN=[0-9a-f]{48}", "SARINA_SERVICE_TOKEN=<TOKEN>", t)
    return t


VISUDO = [("visudo", {"sh": 'printf "  checked: " >> "$W/calls"; cat "$2" >> "$W/calls"\n'})]
GATE_FILES = {"/usr/local/sbin/labyrinth-agent": ("#!/bin/sh\n", 0o755), "/usr/local/sbin/labyrinth-agent-window": ("#!/bin/sh\n", 0o755),
              "/usr/local/bin/": ("dir", 0o755), "/etc/sudoers.d/": ("dir", 0o750)}
USERS = "tester:x:1001:1001::/home/tester:/bin/bash\nbin:x:1:1::/:/usr/bin/nologin\n"
GROUPS = "tester:x:1001:\n"

# ---------------------------------------------------------------- labyrinth-agent-setup

AS = "labyrinth-agent-setup"
USERADD = 'n=; for a in "$@"; do n=$a; done; echo "$n:x:1002:1002::/home/$n:/bin/bash" >> /etc/passwd\n'


def agent_setup(name, steps, files=None, fakes=(), passwd=USERS, env=None, differs=()):
    f = dict(GATE_FILES)
    f.update(files or {})
    for k, v in list(f.items()):
        if v is None:
            del f[k]
    setup = C.world(files=f, stubs=("useradd", "passwd", "visudo", "systemctl"), fakes=VISUDO + [("useradd", {"sh": USERADD})] + list(fakes),
                    passwd=passwd, group=GROUPS)
    C.case(AS, name, steps, setup, env=dict({"SUDO_USER": "tester"}, **(env or {})), differs=differs, norm_more=norm)


HEIMDALL = '{\n  "api_addr": "127.0.0.1:8930",\n  "ratio": 1.5,\n  "name": "Ä \\u00e9",\n  "nested": {"a": [1, 2.0, null, true]}\n}\n'
agent_setup("help", [{"args": ["-h"]}, {"args": ["--help"]}])
agent_setup("usage errors", [{"args": ["--bogus"]}, {"args": ["x"]}, {"args": ["--agent", "Bad"]}, {"args": ["--agent", "a"]},
                             {"args": ["--agent", "a_b"]}, {"args": ["--agent", "-x"]}])
# bash stopped on the unbound $2 (exit code 1); the port: a usage error (2)
agent_setup("--owner/--agent without a value", [{"args": ["--owner"]}, {"args": ["--agent"]}], differs=(("err", 0), ("err", 1), ("rc", 0), ("rc", 1)))
agent_setup("no owner", [{"args": []}], env={"SUDO_USER": None})
agent_setup("no such owner", [{"args": ["--owner", "nobody2"]}])
agent_setup("owner names that are not account names (fixed: refused)", [{"args": ["--owner", "1001"]}, {"args": ["--owner", "a b"]},
                                                                       {"args": ["--owner", "x\ny ALL=(ALL) ALL"]}, {"args": ["--owner", "%wheel"]},
                                                                       {"args": ["--owner", "é"]}],
            passwd=USERS + "1001:x:1003:1003::/:/bin/sh\n")
agent_setup("the agent is a system account or the owner (fixed: refused)", [{"args": ["--agent", "bin"]}, {"args": ["--agent", "root"]},
                                                                           {"args": ["--owner", "tester", "--agent", "tester"]}])
agent_setup("first set-up: the account made, rules, claude-sudo, Heimdall told", [{"args": []}],
            files={"/etc/heimdall/config.json": HEIMDALL})
agent_setup("again: the account there, --owner-login, Heimdall knows them", [{"args": ["--owner-login"]}],
            files={"/etc/heimdall/config.json": '{"maintenance_actors": ["claude-agent", "tester"]}'},
            passwd=USERS + "claude-agent:x:1002:1002::/home/claude-agent:/bin/bash\n")
agent_setup("another agent name and owner", [{"args": ["--owner", "tester", "--agent", "helper-2"]}])
agent_setup("Heimdall's file is not JSON", [{"args": []}],
            files={"/etc/heimdall/config.json": "{not json"}, differs=(("err", 0),))
agent_setup("Heimdall's maintenance_actors is not a list", [{"args": []}],
            files={"/etc/heimdall/config.json": '{"maintenance_actors": 5}'}, differs=(("err", 0),))
agent_setup("Heimdall's file is a list", [{"args": []}], files={"/etc/heimdall/config.json": "[1]"}, differs=(("err", 0),))
agent_setup("the gate is missing", [{"args": []}], files={"/usr/local/sbin/labyrinth-agent": None})
agent_setup("the window is not executable", [{"args": []}], files={"/usr/local/sbin/labyrinth-agent-window": ("#!/bin/sh\n", 0o644)})
agent_setup("visudo refuses", [{"args": []}], fakes=[("visudo", {"rc": 1, "err": "parse error\n"})])
agent_setup("useradd fails", [{"args": []}], fakes=[("useradd", {"rc": 9, "err": "useradd: exists\n"})])
agent_setup("passwd fails", [{"args": []}], fakes=[("passwd", {"rc": 3, "err": "passwd: no\n"})])
agent_setup("remove", [{"args": ["--remove"]}, {"args": ["--remove", "--agent", "other"]}],
            files={"/etc/sudoers.d/50-labyrinth-agent": ("x\n", 0o440), "/usr/local/bin/claude-sudo": ("x\n", 0o755)})
agent_setup("not root", [{"args": [], "user": "1001"}])

# ---------------------------------------------------------------- labyrinth-agent-window

AW = "labyrinth-agent-window"
TIMERS = "Wed 2026-11-11 20:00:00 MST 3h 59min left - - labyrinth-agent-window-close.timer labyrinth-agent-window-close.service\n"


def window(name, steps, files=None, fakes=(), env=None, differs=()):
    f = {"/etc/sudoers.d/": ("dir", 0o750)}
    f.update(files or {})
    setup = C.world(files=f, stubs=("logger", "visudo", "systemctl", "systemd-run"), fakes=VISUDO + list(fakes), passwd=USERS, group=GROUPS)
    C.case(AW, name, steps, setup, env=dict({"SUDO_USER": "tester"}, **(env or {})), differs=differs, norm_more=norm)


OPEN = {"/etc/sudoers.d/51-labyrinth-agent-window": ("rules\n", 0o440)}
window("open for 4 hours (the default), then the status, then close", [{"args": []}, {"args": ["--status"]}, {"args": ["--close"]}, {"args": ["--status"]}],
       fakes=[("systemctl_list-timers", {"out": TIMERS})])
window("hours", [{"args": ["1"]}, {"args": ["12"]}, {"args": [""]}, {"args": ["9", "extra"]}])
window("bad hours", [{"args": ["0"]}, {"args": ["13"]}, {"args": ["01"]}, {"args": ["1 "]}, {"args": ["-1"]}, {"args": ["١"]}, {"args": ["4\n"]}])
window("status: open with no timer, or an odd list", [{"args": ["--status"]}], files=OPEN,
       fakes=[("systemctl_list-timers", {"out": "  a\n\nb c d e\n"})])
window("not by the agent, nor without sudo", [{"args": ["4"], "env": {"SUDO_USER": "claude-agent"}}, {"args": ["4"], "env": {"SUDO_USER": ""}},
                                              {"args": ["--close"], "env": {"SUDO_USER": None}}])
window("SUDO_USER that is no account name (fixed: refused)", [{"args": ["4"], "env": {"SUDO_USER": "x\nclaude-agent ALL=(ALL) ALL"}},
                                                            {"args": ["4"], "env": {"SUDO_USER": "a b"}}])
window("visudo refuses", [{"args": ["2"]}], fakes=[("visudo", {"rc": 1, "err": "syntax error\n"})])
window("systemd-run fails (fixed: the rules are removed again)", [{"args": ["2"]}], fakes=[("systemd-run", {"rc": 1, "err": "Unit already loaded\n"})])
window("logger fails", [{"args": ["2"]}, {"args": ["--close"]}], fakes=[("logger", {"rc": 1})])
window("close when closed", [{"args": ["--close"]}])
window("not root", [{"args": ["3"], "user": "1001"}])

# ---------------------------------------------------------------- labyrinth-ecosystem-setup

ES = "labyrinth-ecosystem-setup"
ECO_USERS = USERS + "sarina:x:1500:1500::/var/lib/sarina:/usr/bin/nologin\nheimdall:x:1501:1501::/var/lib/heimdall:/usr/bin/nologin\n"
ECO_GROUPS = GROUPS + "sarina:x:1500:\nheimdall:x:1501:\n"
KEYGEN = 'echo "private" > "$9"; echo "ssh-ed25519 AAAAC3NzaFake validation@labyrinth" > "$9.pub"\n'
IMG = "/var/lib/limbo/images/arch-cloudimg.qcow2"


def eco(name, steps=None, files=None, fakes=(), units=("ollama", "sarina-service", "heimdall", "limbo"), stubs=None, passwd=ECO_USERS, group=ECO_GROUPS,
        differs=()):
    fk = [("ssh-keygen", {"sh": KEYGEN}), ("ollama_list", {"out": "NAME            ID      SIZE\nqwen3:8b        abc     5.2 GB\n"}),
          ("pacman_-Q_labyrinth-ollama-tuning", {"rc": 1})]
    for u in ("ollama", "sarina-service", "heimdall", "limbo"):
        if u not in units:
            fk.append(("systemctl_cat_%s.service" % u, {"rc": 1}))
    setup = C.world(files=files, stubs=stubs or ("systemctl", "pacman", "setfacl", "ssh-keygen", "ollama"), fakes=fk + list(fakes), passwd=passwd,
                    group=group)
    C.case(ES, name, steps or [{"args": []}], setup, differs=differs, norm_more=norm)


eco("help, usage", [{"args": ["-h"]}, {"args": ["--bogus"]}, {"args": ["--model", "a b"]}, {"args": ["--model", "x\nSARINA_SERVICE_TOKEN=known"]}])
eco("--model without a value", [{"args": ["--model"]}], differs=(("err", 0), ("rc", 0)))
eco("everything installed, a base image", files={IMG: "a disk image\n"})
eco("again: everything there already is left alone", [{"args": []}, {"args": ["--model", "qwen3:14b"]}], files={IMG: "a disk image\n"})
eco("nothing installed", units=(), stubs=("systemctl", "pacman"))
eco("--no-limbo-profile, another model, the tuning package", [{"args": ["--no-limbo-profile", "--model", "llama3.1:8b"]}], files={IMG: "x\n"},
    fakes=[("pacman_-Q_labyrinth-ollama-tuning", {"rc": 0})])
eco("no base image", files={})
eco("the key there already, a profile there already", [{"args": []}],
    files={IMG: "x\n", "/etc/limbo/keys/validation": ("old\n", 0o644), "/etc/limbo/keys/validation.pub": "ssh-ed25519 OLD x\n",
           "/etc/limbo/validation/arch-validate.json": "{}\n"})
eco("the key there without its public half (fixed: stops)", [{"args": []}], files={IMG: "x\n", "/etc/limbo/keys/validation": ("old\n", 0o600)},
    differs=(("err", 0),))
eco("no sarina group", passwd=USERS, group=GROUPS, differs=(("err", 0),))
eco("enable fails, models missing, no setfacl", fakes=[("systemctl_enable", {"rc": 1}), ("ollama_list", {"out": "NAME ID\nqwen3:4b x\n"})],
    stubs=("systemctl", "pacman", "ssh-keygen", "ollama"))
eco("ollama list fails", fakes=[("ollama_list", {"rc": 1, "err": "could not connect\n"})])
eco("daemon-reload fails", fakes=[("systemctl_daemon-reload", {"rc": 4, "err": "Failed\n"})])
eco("not root", [{"args": [], "user": "1001"}])

# ---------------------------------------------------------------- labyrinth-hardware-setup

HW = "labyrinth-hardware-setup"


def pci(devices):
    out = {}
    for i, (cls, vendor) in enumerate(devices):
        d = "/fakesys/bus/pci/devices/0000:0%d:00.0/" % i
        out[d + "class"] = cls + "\n"
        out[d + "vendor"] = vendor + "\n"
    return out


def hw(name, devices, vendor="Micro-Star International Co., Ltd.\n", installed=(), steps=None, fakes=()):
    f = pci(devices)
    if vendor is not None:
        f["/fakesys/class/dmi/id/sys_vendor"] = vendor
    fk = [("pacman_-Q", {"rc": 1})] + [("pacman_-Q_" + p, {"rc": 0}) for p in installed] + list(fakes)
    setup = C.world(files=f, stubs=("pacman", "systemctl"), fakes=fk)
    C.case(HW, name, steps or [{"args": []}, {"args": ["--dry-run"]}], setup, env={"LABYRINTH_HW_SYS": "/fakesys"})


NV, INTEL, AMD, NET = ("0x030000", "0x10de"), ("0x030000", "0x8086"), ("0x038000", "0x1002"), ("0x020000", "0x10de")
ALLPK = ("linux-cachyos-nvidia-open", "linux-cachyos-lts-nvidia-open", "lib32-nvidia-utils", "nvidia-settings", "nvidia-prime", "nvidia-utils",
         "rog-control-center", "asusctl")
hw("NVIDIA only", [NV, NET], installed=ALLPK)
hw("NVIDIA and Intel: switching", [INTEL, NV], installed=ALLPK)
hw("AMD only: NVIDIA's packages go", [AMD], installed=ALLPK)
hw("an ASUS laptop with AMD and NVIDIA", [AMD, NV], vendor="ASUSTeK COMPUTER INC.\n", installed=ALLPK)
hw("nothing to remove, no DMI", [INTEL], vendor=None)
hw("a network card of NVIDIA's is no graphics", [NET], installed=("nvidia-utils",))
hw("--dry-run only, another argument ignored", [NV], steps=[{"args": ["--dry-run", "x"]}, {"args": ["x", "--dry-run"]}])
hw("the commands fail (their output in the log)", [AMD], installed=ALLPK,
   fakes=[("pacman_-Rns", {"rc": 1, "out": "removing\n", "err": "error: failed\n"}), ("systemctl", {"rc": 1, "err": "no\n"})])
hw("no PCI folder", [])

# ---------------------------------------------------------------- labyrinth-grub-theme

GT = "labyrinth-grub-theme"
THEME_SH = r"""#!/bin/sh
# a stand-in for grub-evodevo's install.sh: says what it was given, writes a menu and the icons
echo "install.sh $*: $(grep -E '^(ScreenWidth|ScreenHeight|FontSize|Title|MenuBg|FocusOpacity|Wallpaper)=' ./install.sh | tr '\n' ' ')" >> @W@/calls
if [ "$1" = -preview ]; then echo png > preview.png; exit 0; fi
mkdir -p /usr/share/grub/themes/evodevo/icons
for n in 1 2 3 4; do echo "icon $n" > /usr/share/grub/themes/evodevo/icons/evodevo_entry_$n.png; done
cat > /boot/grub/grub.cfg <<'X'
menuentry 'Labyrinth OS' --class labyrinth --class gnu-linux --class gnu --class os --class evodevo_entry_1 $menuentry_id_option 'gnulinux' {
	initrd /boot/initramfs.img
}
	menuentry 'Windows Boot Manager (on /dev/sda1)' --class windows --class os --class evodevo_entry_2 $menuentry_id_option 'osprober' {
}
menuentry 'UEFI Firmware Settings' --class evodevo_entry_3 --class ../../../../etc/evil --class uefi {
}
menuentry 'evodevo_entry_4 title' --class evodevo_entry_4	{
}
menuentry 'Labyrinth, LTS' --class labyrinth --class evodevo_entry_1 {
}
X
echo "theme installed"
"""
INSTALL_VARS = ("#!/bin/sh\nScreenWidth=1280\nScreenHeight=720\nFontSize=16\nWallpaper='x'\nTitle='Evodevo'\nMenuBg='0 0 0'\nHeaderBg=1\nFocusBg=1\n"
                "TextFg=1\nIconFg=1\nBarBg=1\nBarFg=1\nMenuOpacity=1\nHeaderOpacity=1\nFocusOpacity=1\nScreenWidth=$((ScreenWidth * 2))\n")


def theme(name, steps=None, files=None, sysfs=None, install=None, fakes=(), differs=()):
    f = {"/usr/share/labyrinth/grub-evodevo/install.sh": ((install or INSTALL_VARS) + THEME_SH.replace("@W@", W).split("\n", 1)[1], 0o755),
         "/usr/share/labyrinth/grub-evodevo/README.md": "MIT\n", "/usr/share/labyrinth/grub-evodevo/icons/labyrinth.svg": "<svg/>\n",
         "/usr/share/grub/themes/.keep": "", "/etc/default/grub": "GRUB_TIMEOUT=2\n#GRUB_GFXMODE=auto\n",
         "/boot/grub/grub.cfg": "menuentry 'old' {\n initrd x\n}\n"}
    for k, v in (files or {}).items():
        if v is None:
            f.pop(k, None)
        else:
            f[k] = v
    for conn, (status, modes) in (sysfs or {}).items():
        f["/sys/class/drm/%s/status" % conn] = status
        f["/sys/class/drm/%s/modes" % conn] = modes
    setup = C.world(files=f, stubs=(), fakes=list(fakes))
    C.case(GT, name, steps or [{"args": []}], setup, differs=differs)


theme("install at 1920x1080 (no monitor read)")
theme("a connected 3440x1440 monitor, a disconnected one first", sysfs={"card1-DP-1": ("disconnected\n", "1920x1080\n"),
                                                                       "card1-DP-2": ("connected\n", "3440x1440\n2560x1080\n"),
                                                                       "card1-HDMI-A-1": ("connected\n", "800x600\n")})
theme("an odd mode is passed over, a mode with a letter", sysfs={"card0-DP-1": ("connected\n", "x1080\n"), "card0-DP-2": ("connected\n", "2560x1440i\n")})
theme("no GFXMODE line, no newline at the end (fixed: a line of its own)", files={"/etc/default/grub": "GRUB_TIMEOUT=2"})
theme("GFXMODE and GFXPAYLOAD already set", files={"/etc/default/grub": "GRUB_GFXMODE=1024x768\nGRUB_GFXPAYLOAD_LINUX=text\n#GRUB_GFXMODE=x\n"})
theme("the theme's install fails: the old menu back", files={"/usr/share/labyrinth/grub-evodevo/install.sh": (
    "#!/bin/sh\nfor i in 1 2 3 4 5 6 7; do echo line $i; done\necho broken >&2\necho 'menuentry x' > /boot/grub/grub.cfg\nexit 3", 0o755)})
theme("the theme writes no menu", files={"/usr/share/labyrinth/grub-evodevo/install.sh": ("#!/bin/sh\necho 'set x' > /boot/grub/grub.cfg\n", 0o755)})
theme("an icon already there is kept", files={"/usr/share/grub/themes/evodevo/icons/windows.png": "mine\n"})
theme("preview, relative and absolute (fixed: the relative one is kept)", [{"args": ["--preview", "menu.png"], "cwd": "/root"},
                                                                         {"args": ["--preview", "/root/abs.png"]}])
theme("preview into a folder", [{"args": ["--preview", "/root"]}])
theme("preview: the theme makes another picture", [{"args": ["--preview", "/root/p.png"]}],
      files={"/usr/share/labyrinth/grub-evodevo/install.sh": ("#!/bin/sh\necho x > shot.png\n", 0o755)})
theme("preview: no picture", [{"args": ["--preview", "/root/p.png"]}],
      files={"/usr/share/labyrinth/grub-evodevo/install.sh": ("#!/bin/sh\nexit 0\n", 0o755)}, differs=(("err", 0),))
theme("--preview without a file", [{"args": ["--preview"]}, {"args": ["--preview", ""]}], differs=(("err", 0), ("err", 1)))
theme("no /etc/default/grub", files={"/etc/default/grub": None}, differs=(("err", 0),))
theme("not root", [{"args": [], "user": "1001"}])
