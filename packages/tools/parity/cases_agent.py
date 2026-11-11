"""The cases of labyrinth-agent (loaded by compare.py), the gate sudo lets the engineering agent run as root.

Each run is in a world of its own: a user and mount namespace (unshare, no real root) where uid 0 is this account and uids
1000-2999 are its subordinate ids (/etc/subuid), so deploy's caller (uid 2001) is another account. /etc, /usr/bin, /usr/share,
/usr/lib and /usr/local are overlays: the stand-ins for pacman, systemctl, journalctl, logger, heimdalld, limbo, ufw and
/usr/local/bin/labyrinth-rollback are files of the world there (the gate finds them in /usr/bin, as it would the real ones), and
every place deploy writes to or the gate searches (the deploy folders, systemd's unit folders, pacman's hooks, udev's rules,
polkit's actions, sudoers.d, /run/systemd/system) is a folder of the world (opaque: nothing of this computer shows through).
Nothing on the computer is changed and none of the real programs runs.

Each stand-in writes down its arguments and its whole environment (calls, and argv.log byte for byte). Beside the comparison of
the two programs, every case is checked against an allow-list written here (`allowed`, a third copy, apart from the script's
and the Rust checks): no call of a stand-in that would act as root outside it, the environment the stand-ins saw clean, and no
file written outside deploy's places. Random command lines (LABYRINTH_AGENT_RANDOM, default 400) are run the same way."""
import os
import random
import stat

import __main__ as P

W = P.WORLD
AGENT = "labyrinth-agent"
FAKES = ("pacman", "systemctl", "journalctl", "logger", "heimdalld", "limbo", "ufw")
ROLLBACK = "/usr/local/bin/labyrinth-rollback"
CALLER = "2001"


def subordinate(path):
    """The first block of 2000 or more subordinate ids this account has (/etc/subuid, /etc/subgid), else None."""
    import pwd
    name = pwd.getpwuid(os.getuid()).pw_name
    try:
        for line in open(path):
            parts = line.strip().split(":")
            if len(parts) == 3 and parts[0] in (name, str(os.getuid())) and int(parts[2]) >= 2000:
                return int(parts[1])
    except OSError:
        pass
    return None


SUBUID, SUBGID = subordinate("/etc/subuid"), subordinate("/etc/subgid")
WRAP = ["/usr/bin/unshare", "--map-root-user", "--map-users=1000:%s:2000" % SUBUID, "--map-groups=1000:%s:2000" % SUBGID,
        "-m", "-u", "--", "/bin/sh", os.path.join(W, "ns.sh")]
# not root: the caller itself, running a copy of the program (it cannot open the script in this account's home folder)
AS_CALLER = ["/bin/sh", "-c", 'cp "$1" "%s/ov/prog"; shift; exec /usr/bin/setpriv --reuid=%s --regid=%s --clear-groups -- "%s/ov/prog" "$@"'
             % (W, CALLER, CALLER, W), "sh"]

FAKE = r"""#!/bin/sh
W=@WORLD@
n=${0##*/}
line=$n
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$W/calls"
printf '  env: %s\n' "$(tr '\0' '\n' < /proc/$$/environ | LC_ALL=C sort | tr '\n' ' ')" >> "$W/calls"
set -- "$n" "$@"
{ printf '%s\0' "$#" "$@"; printf '%s\0' "$(tr -cd '\0' < /proc/$$/environ | wc -c)"; LC_ALL=C sort -z < /proc/$$/environ; } >> "$W/argv.log"
shift
rc=$(cat "$W/fake/rc.$n" 2>/dev/null || echo 0)
case "$n" in
  pacman)
    case "$1" in
      -Qip) if [ -f "$2" ]; then cat "$2"; else echo "error: '$2': cannot open" >&2; exit 1; fi; exit 0 ;;
      -Si) if grep -qxF -- "$2" "$W/fake/repo"; then printf 'Repository : extra\nName : %s\n' "$2"; exit 0; fi
           echo "error: package '$2' was not found" >&2; exit 1 ;;
      -S) if [ "$2" = --print ]; then
            [ -e "$W/fake/sp-fails" ] && { echo "error: target not found" >&2; exit 1; }
            shift 6
            for t in "$@"; do printf '%s\n' "$t"; cat "$W/fake/deps/$t" 2>/dev/null; done
            exit 0
          fi ;;
    esac ;;
  logger) exit 0 ;;
esac
echo "$n: done"
[ "$rc" = TERM ] && kill -TERM $$
exit "$rc"
"""

# deploy's places, and where root is told to run programs: folders of the world (opaque)
OPAQUE = {
    "/usr/share": ["yggdrasil", "labyrinth", "icons", "applications", "wayland-sessions", "libalpm/hooks", "polkit-1/actions"],
    "/etc": ["yggdrasil", "systemd/system", "pacman.d/hooks", "udev/rules.d", "sudoers.d"],
    "/usr/lib": ["systemd/system", "udev/rules.d"],
    "/usr/local": ["lib/systemd/system"],
    "/usr/bin": [],
}
UPPERS = ("/usr/bin", "/usr/share", "/etc", "/usr/local")

ENV = {"SUDO_USER": "tester", "SUDO_UID": CALLER, "SUDO_GID": CALLER, "SUDO_COMMAND": "/usr/bin/labyrinth-agent", "USER": "root",
       "LOGNAME": "root", "TERM": "xterm-256color"}


def pkg(name_line):
    return "Name            : %s\nVersion         : 1-1\nDescription     : made up\n" % name_line if name_line is not None else "no name here\n"


STANDARD_PKGS = {"yggdrasil-a-1-1-x86_64.pkg.tar.zst": pkg("yggdrasil-a"), "heimdall-git-1-1-x86_64.pkg.tar.xz": pkg("heimdall-git"),
                 "openssh-1-1-x86_64.pkg.tar.zst": pkg("openssh"), "sudo-1-1-x86_64.pkg.tar.zst": pkg("sudo"),
                 "evil-yggdrasil-1-1-x86_64.pkg.tar.zst": pkg("evil-yggdrasil"), "noname-1-1-x86_64.pkg.tar.zst": pkg(None)}
STANDARD_REPO = ["htop", "gnu-efi", "sshfs", "remmina", "tailscale", "python-lsp", "fuse3"]
STANDARD_DEPS = {"htop": ["ncurses"], "sshfs": ["fuse3", "openssh"], "remmina": ["freerdp", "gnome-remote-desktop"],
                 "python-lsp": ["python-lsp-server"]}
STANDARD_SRC = {"pub": ("program\n", 0o644), "secret": ("root only\n", 0o600), "exe": ("#!/bin/sh\necho hi\n", 0o755)}
STANDARD_UNITS = {"/usr/lib/systemd/system/yggdrasil-spell-global.service": "[Service]\nExecStart=/usr/bin/yggdrasil-spell global merge\n",
                  "/usr/lib/systemd/system/yggdrasil-gametime-catch-up.service": "[Service]\nExecStart=/usr/bin/yggdrasil-gametime catch-up --system\n",
                  "/usr/lib/systemd/system/yggdrasil-x.service": "[Unit]\nRequires=yggdrasil-spell-global.service\n[Service]\nExecStart=/usr/bin/true\n"}


def upper(mount, rel=""):
    return os.path.join(W, "ov" + mount, "up", rel)


def world(pkgs=None, repo=None, deps=None, sp_fails=False, rc=None, src=None, units=None, existing=None, links=None,
          etc_yggdrasil=True, run_units=None, extra=None):
    pkgs = STANDARD_PKGS if pkgs is None else pkgs
    repo = STANDARD_REPO if repo is None else repo
    deps = STANDARD_DEPS if deps is None else deps
    src = STANDARD_SRC if src is None else src
    units = STANDARD_UNITS if units is None else units

    def go(paths):
        for m, dirs in OPAQUE.items():
            os.makedirs(upper(m), exist_ok=True)
            os.makedirs(os.path.join(W, "ov" + m, "work"), exist_ok=True)
            for d in dirs:
                if m == "/etc" and d == "yggdrasil" and not etc_yggdrasil:
                    continue
                os.makedirs(upper(m, d), exist_ok=True)
                os.setxattr(upper(m, d), "user.overlay.opaque", b"y")
        for f in FAKES:
            P.write(upper("/usr/bin", f), FAKE.replace("@WORLD@", W), 0o755)
        P.write(upper("/usr/local", "bin/labyrinth-rollback"), FAKE.replace("@WORLD@", W), 0o755)
        P.write(os.path.join(W, "fake", "repo"), "".join(n + "\n" for n in repo))
        for n, ds in deps.items():
            P.write(os.path.join(W, "fake", "deps", n), "".join(d + "\n" for d in ds))
        if sp_fails:
            P.write(os.path.join(W, "fake", "sp-fails"), "")
        for n, v in (rc or {}).items():
            P.write(os.path.join(W, "fake", "rc." + n), str(v))
        for n, body in pkgs.items():
            P.write(os.path.join(W, "pkgs", n), body)
        for n, (body, mode) in src.items():
            P.write(os.path.join(W, "src", n), body, mode)
        for path, body in list(units.items()) + list((existing or {}).items()):
            m = next(m for m in ("/usr/bin", "/usr/share", "/etc", "/usr/lib", "/usr/local") if path.startswith(m + "/"))
            P.write(upper(m, path[len(m) + 1:]), body[0] if isinstance(body, tuple) else body, body[1] if isinstance(body, tuple) else 0o644)
        for path, target in (links or {}).items():
            m = next(m for m in ("/usr/bin", "/usr/share", "/etc", "/usr/lib", "/usr/local") if path.startswith(m + "/"))
            p = upper(m, path[len(m) + 1:])
            os.makedirs(os.path.dirname(p), exist_ok=True)
            os.symlink(target, p)
        os.makedirs(os.path.join(W, "run-units"), exist_ok=True)
        for n, body in (run_units or {}).items():
            P.write(os.path.join(W, "run-units", n), body)
        os.chmod(os.path.join(W, "src"), 0o755) if os.path.isdir(os.path.join(W, "src")) else None
        if extra:
            extra()
        mounts = [("hostname", "parity-host")]
        if not etc_yggdrasil:
            mounts.append(("whiteout", "/etc", "yggdrasil"))
        mounts += [("overlay", m) for m in OPAQUE]
        mounts.append(("bind", os.path.join(W, "run-units"), "/run/systemd/system"))
        P.world_of_its_own(mounts)
        BEFORE.clear()
        BEFORE.update(upper_files())
    return go


BEFORE = {}


def upper_files():
    """The files of the overlays' upper folders, with mode and contents (the stand-ins left out)."""
    out = {}
    for m in UPPERS:
        root = upper(m)
        for d, dirs, files in os.walk(root):
            for n in files + [x for x in dirs if os.path.islink(os.path.join(d, x))]:
                p = os.path.join(d, n)
                rel = m + "/" + os.path.relpath(p, root)
                if (m == "/usr/bin" and n in FAKES) or rel == ROLLBACK:
                    continue
                st = os.lstat(p)
                if stat.S_ISLNK(st.st_mode):
                    out[rel] = ("link", os.fsencode(os.readlink(p)))
                elif stat.S_ISREG(st.st_mode):
                    out[rel] = (oct(st.st_mode & 0o7777), open(p, "rb").read())
                else:
                    out[rel] = (oct(st.st_mode), b"")
            for n in dirs:
                p = os.path.join(d, n)
                if not os.path.islink(p):
                    out[m + "/" + os.path.relpath(p, root) + "/"] = (oct(os.stat(p).st_mode & 0o7777), b"")
    return out


def snap():
    return {"uppers": {k: [v[0], v[1].decode("utf-8", "backslashreplace")] for k, v in upper_files().items()}}


# ---------------------------------------------------------------- the allow-list of these cases (apart from both programs')

ECO = ("yggdrasil", "labyrinth", "heimdall", "sarina", "limbo", "liki", "agora")
REMOTE = set("tailscale headscale ngrok frp frpc frps chisel rathole bore bore-cli meshagent tor openssh dropbear tinyssh tigervnc x11vnc "
             "wayvnc xrdp gnome-remote-desktop krfb krdp ttyd cockpit sunshine samba ksmbd-tools nfs-utils vsftpd".split())
REMOTE_FAMILIES = ("zerotier", "netbird", "cloudflared", "anydesk", "teamviewer", "rustdesk")
CLEAN_ENV = {"PATH", "HOME", "LOGNAME", "USER", "SUDO_COMMAND", "SUDO_GID", "SUDO_HOME", "SUDO_UID", "SUDO_USER", "TERM"}


def remote(n):
    return n in REMOTE or n.startswith(REMOTE_FAMILIES) or n.endswith("-server")


def target(n):
    return 1 <= len(n) <= 100 and not n.startswith("-") and all(c in "abcdefghijklmnopqrstuvwxyz0123456789@._+-" for c in n)


def package_file(p):
    return p.startswith("/") and ".." not in p.split("/") and p.endswith((".pkg.tar.zst", ".pkg.tar.xz"))


def eco_package_in(p):
    """the package name in what the stand-in pacman -Qip prints for this file (pacman's "Name :" field) is an ecosystem name"""
    import re
    try:
        text = open(p, encoding="utf-8", newline=None).read()
    except (OSError, UnicodeDecodeError):
        return False
    m = re.search(r"^Name\s*:\s*(\S+)", text, re.M)
    n = m.group(1) if m else ""
    return n.startswith(ECO) and all(c in "abcdefghijklmnopqrstuvwxyz0123456789-._+" for c in n)


def journalctl_ok(args):
    """journalctl (systemd 262) reading its arguments: only the agent's read-only options in an option's place."""
    none = {"no-pager", "dmesg", "quiet", "follow", "reverse", "catalog", "no-hostname", "all", "case-sensitive"}
    req = {"unit", "output", "since", "until", "priority", "identifier", "grep"}
    fields = {"_COMM", "_PID", "_UID", "_TRANSPORT", "_SYSTEMD_UNIT", "_SYSTEMD_USER_UNIT", "SYSLOG_IDENTIFIER", "PRIORITY"}

    def lines(w):
        d = w[1:] if w.startswith("+") else w
        return w == "all" or (d.isascii() and d.isdigit())

    def boot(w):
        d = w[1:] if w.startswith("-") else w
        return (d.isascii() and d.isdigit()) or (len(w) == 32 and all(c in "0123456789abcdefABCDEF" for c in w))

    i = 0
    while i < len(args):
        a = args[i]
        i += 1
        if a == "--":
            return "journalctl --"
        if a.startswith("--"):
            name, eq, _ = a[2:].partition("=")
            if name in req:
                i += 0 if eq else 1
            elif name in ("lines", "boot"):
                if not eq and i < len(args) and (lines if name == "lines" else boot)(args[i]):
                    i += 1
            elif name not in none or (eq and name != "case-sensitive"):
                return "journalctl option " + a
        elif len(a) > 1 and a[0] == "-":
            for k, c in enumerate(a[1:]):
                rest = a[2 + k:]
                if c in "kqfrxa":
                    continue
                if c in "uoptg":
                    i += 0 if rest else 1
                    break
                if c in "nb":
                    if not rest and i < len(args) and (lines if c == "n" else boot)(args[i]):
                        i += 1
                    break
                return "journalctl option -%s in %s" % (c, a)
        else:
            f, eq, v = a.partition("=")
            if not (eq and f in fields and all(c.isascii() and (c.isalnum() or c in "@._:/-") for c in v)):
                return "journalctl match " + a
    return None


def call_ok(argv):
    """None when this call of a stand-in is one the gate may make, else why not."""
    n, a = argv[0], argv[1:]
    if n == "logger":
        return None if len(a) == 6 and a[:5] == ["-t", "labyrinth-agent", "-p", "auth.notice", "--"] else "logger %r" % a
    if n == "pacman":
        if a[:1] == ["-Qip"] and len(a) == 2 and package_file(a[1]):
            return None
        if a[:1] == ["-Si"] and len(a) == 2 and target(a[1]):
            return None
        if a[:6] == ["-S", "--print", "--print-format", "%n", "--needed", "--noconfirm"] and a[6:] and all(map(target, a[6:])):
            return None
        if a[:2] == ["-U", "--noconfirm"] and a[2:]:
            bad = [f for f in a[2:] if not (package_file(f) and os.path.isfile(f) and not os.path.islink(f) and eco_package_in(f))]
            return "pacman -U %r" % bad if bad else None
        if a[:3] == ["-S", "--needed", "--noconfirm"] and a[3:]:
            repo = open(os.path.join(W, "fake", "repo")).read().split("\n")
            for t in a[3:]:
                if not target(t) or remote(t) or t not in repo:
                    return "pacman -S " + t
                deps = os.path.join(W, "fake", "deps", t)
                if os.path.exists(deps) and any(remote(d) for d in open(deps).read().split()):
                    return "pacman -S %s brings a remote-access tool" % t
            return None
        return "pacman %r" % a
    if n == "systemctl":
        if a == ["daemon-reload"]:
            return None
        verbs = {("enable", "--now"): "enable-now", ("disable", "--now"): "disable-now", ("--global", "enable"): "user-enable",
                 ("--global", "disable"): "user-disable"}
        if tuple(a[:2]) in verbs:
            verb, units = verbs[tuple(a[:2])], a[2:]
        elif a[:1] and a[0] in ("start", "stop", "restart", "reload", "status", "is-active", "enable", "disable"):
            verb, units = a[0], a[1:]
        else:
            return "systemctl %r" % a
        heads = ("yggdrasil-", "labyrinth-", "heimdall", "limbo", "sarina", "ollama", "systemd-sysctl", "greetd-test")
        for u in units:
            if not (u.startswith(heads) and all(c.isascii() and (c.isalnum() or c in "@._:-") for c in u)):
                return "systemctl unit " + u
            if u.startswith("labyrinth-agent") or (u.startswith("heimdall") and verb in ("stop", "disable", "disable-now", "user-disable")):
                return "systemctl %s %s" % (verb, u)
        return None if units else "systemctl without a unit"
    if n == "journalctl":
        return journalctl_ok(a[1:]) if a[:1] == ["--no-pager"] else "journalctl without --no-pager"
    if n == "heimdalld":
        if a[:1] != ["maintenance"] or a[1:2] not in (["on"], ["off"], ["status"]):
            return "heimdalld %r" % a
        i = 2
        while i < len(a):
            w = a[i]
            name = w.lstrip("-").partition("=")[0] if w.startswith("-") and len(w) > 1 else None
            if name not in ("for", "reason") or w.startswith("---"):
                return "heimdalld word " + w
            i += 1 if "=" in w else 2
        return None
    if n == "limbo":
        def manifest(m):
            return m.startswith(("/var/lib/limbo/build-agent/", "/etc/limbo/validation/")) and m.endswith(".json") and ".." not in m.split("/")
        if (len(a) == 2 and a[0] in ("up", "down") and manifest(a[1])) or (a[:1] in (["doctor"], ["status"]) and (len(a) == 1 or (len(a) == 2 and manifest(a[1])))):
            return None
        return "limbo %r" % a
    if n == "ufw":
        return None if a == ["status", "verbose"] else "ufw %r" % a
    if n == "labyrinth-rollback":
        return None if a in (["list"], ["status"]) or (len(a) == 2 and a[1] == "--dry-run" and a[0].isascii() and a[0].isdigit()) else "rollback %r" % a
    return "a program %r" % n


def deploy_ok(path, mode, units_text):
    """None when the gate may have written this file."""
    parts = path.split("/")
    if not path.startswith("/") or any(p in ("", ".", "..") for p in parts[1:]):
        return "unclean " + path
    for never in ("/usr/share/yggdrasil/gpu-unlock", "/usr/share/labyrinth/pkgbuilds", "/usr/share/labyrinth/grub-evodevo"):
        if path == never or path.startswith(never + "/"):
            return "a place root uses: " + path
    name = parts[-1]

    def plain(x, slash):
        return x != "" and all(c.isascii() and (c.isalnum() or c in "._-" or (slash and c == "/")) for c in x)

    if path.startswith("/usr/bin/"):
        p = path[9:]
        prog = p == "minos" or any(p.startswith(h) and p[len(h):] and all(c in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in p[len(h):])
                                   for h in ("yggdrasil-", "ygg-"))
        if not prog or mode != "0o755":
            return "program " + path
        import re
        if re.search(r"(?<![A-Za-z0-9._@+-])" + re.escape(name) + r"(?![A-Za-z0-9._@+-])", units_text):
            return "a program root runs: " + path
        return None
    ok = mode == "0o644" and (
        any(path.startswith(d) and plain(path[len(d):], True) for d in ("/usr/share/yggdrasil/", "/usr/share/labyrinth/"))
        or (path.startswith("/usr/share/icons/") and path.endswith((".svg", ".png")) and plain(path[17:-4], True))
        or any(path.startswith(d) and path.endswith(".desktop") and plain(path[len(d):-8], False)
               for d in ("/usr/share/applications/", "/usr/share/wayland-sessions/"))
        or (path.startswith("/etc/yggdrasil/") and path.endswith((".json", ".conf")) and plain(path[15:-5], False)))
    return None if ok else "a place the agent may not write: %s (%s)" % (path, mode)


def parse_argv_log():
    """[(argv, env)] from argv.log."""
    p = os.path.join(W, "argv.log")
    if not os.path.exists(p):
        return []
    data = open(p, "rb").read().split(b"\0")
    out, i = [], 0
    while i < len(data) - 1:
        n = int(data[i])
        argv = [x.decode("utf-8", "surrogateescape") for x in data[i + 1:i + 1 + n]]
        i += 1 + n
        e = int(data[i])
        env = [x.decode("utf-8", "surrogateescape") for x in data[i + 1:i + 1 + e]]
        i += 1 + e
        out.append((argv, env))
    return out


def check(results, calls, tree):
    """Everything this run did against the allow-list; a list of what is outside it."""
    problems = []
    for argv, env in parse_argv_log():
        why = call_ok(argv)
        if why:
            problems.append("call outside the allow-list: " + why)
        keys = {e.partition("=")[0] for e in env}
        if keys - CLEAN_ENV or "PATH=/usr/bin" not in env or "HOME=/root" not in env:
            problems.append("%s saw the environment %r" % (argv[0], sorted(env)))
    # what root reads programs from: the files in these places and in their subfolders (systemd's drop-ins and .wants; pacman,
    # udev and polkit read only the folder itself, sudo includes sudoers.d's files)
    places = ("/etc/systemd/system/", "/usr/local/lib/systemd/system/", "/usr/lib/systemd/system/", "/etc/pacman.d/hooks/",
              "/usr/share/libalpm/hooks/", "/etc/udev/rules.d/", "/usr/lib/udev/rules.d/", "/usr/share/polkit-1/actions/", "/etc/sudoers.d/")
    units_text = "\n".join(v[1].decode("utf-8", "replace") for k, v in BEFORE.items()
                           if any(k.startswith(d) and k[len(d):].count("/") <= 1 for d in places) and not k.endswith("/"))
    units_text += "\n".join(open(os.path.join(W, "run-units", n), errors="replace").read() for n in os.listdir(os.path.join(W, "run-units")))
    after = upper_files()
    for path, (mode, body) in after.items():
        if path.endswith("/"):
            continue
        if BEFORE.get(path) != (mode, body):
            why = deploy_ok(path, mode, units_text)
            if why:
                problems.append("file written outside deploy's places: " + why)
    for path in BEFORE:
        if path not in after and not path.endswith("/") and not (path.endswith(".agent-tmp") and not deploy_ok(path[:-10], "0o644", "")):
            problems.append("file removed: " + path)
    return problems


# ---------------------------------------------------------------- the cases


def case(name, steps, env=None, **world_args):
    e = dict(ENV)
    e.update(env or {})
    for s in steps:
        s.setdefault("wrap", WRAP)
    P.case(AGENT, name, steps, setup=world(**world_args), env=e, snap=snap, check=check)


def one(name, *args, env=None, **world_args):
    case(name, [{"args": list(args)}], env=env, **world_args)


def each(name, lines, env=None, **world_args):
    case(name, [{"args": list(a)} for a in lines], env=env, **world_args)


PK = "{WORLD}/pkgs/"
SRC = "{WORLD}/src/"

if SUBUID is None or SUBGID is None:
    print("labyrinth-agent: no subordinate ids for this account in /etc/subuid and /etc/subgid: its cases are left out")
else:
    # -- help, root, unknown
    each("help", [[], ["-h"], ["--help"], ["--help", "x"], ["-h", "service"]])
    case("not root: refused before anything, help still given", [{"args": ["service", "status", "heimdall"], "wrap": WRAP + AS_CALLER},
                                                               {"args": ["--help"], "wrap": WRAP + AS_CALLER},
                                                               {"args": ["deploy", SRC + "pub", "/usr/bin/yggdrasil-x"], "wrap": WRAP + AS_CALLER}])
    each("unknown commands", [["foo"], [""], ["--"], ["PKG-INSTALL"], ["service"], ["deploy"], ["deploy", "a"], ["deploy", "a", "b", "c"],
                              ["limbo"], ["rollback"], ["maintenance"], ["maintenance", "bogus"], ["pkg-install\n"], ["-x"]])

    # -- pkg-install
    one("pkg-install: one of ours", "pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst")
    one("pkg-install: two, .xz too", "pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst", PK + "heimdall-git-1-1-x86_64.pkg.tar.xz")
    one("pkg-install: pacman fails", "pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst", rc={"pacman": 1})
    one("pkg-install: heimdalld fails, pacman still runs", "pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst", rc={"heimdalld": 3})
    one("pkg-install: pacman killed", "pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst", rc={"pacman": "TERM"})
    each("pkg-install: not ours", [["pkg-install", PK + p] for p in ("openssh-1-1-x86_64.pkg.tar.zst", "sudo-1-1-x86_64.pkg.tar.zst",
                                                                     "evil-yggdrasil-1-1-x86_64.pkg.tar.zst", "noname-1-1-x86_64.pkg.tar.zst")])
    names = {"a.pkg.tar.zst": "yggdrasil", "b.pkg.tar.zst": "yggdrasil_x", "c.pkg.tar.zst": "Yggdrasil-x", "d.pkg.tar.zst": "labyrinthine",
             "e.pkg.tar.zst": "heimdallx", "f.pkg.tar.zst": "liki", "g.pkg.tar.zst": "agora+1.0", "h.pkg.tar.zst": "yggdrasil-x extra words",
             "i.pkg.tar.zst": "yggdrasil-é", "j.pkg.tar.zst": "ygg-x", "k.pkg.tar.zst": "sarina@x", "l.pkg.tar.zst": "limbo-git"}
    each("pkg-install: names that look like ours", [["pkg-install", PK + n] for n in sorted(names)], pkgs={n: pkg(v) for n, v in names.items()})
    odd = {"crlf.pkg.tar.zst": "Name : yggdrasil-crlf\r\nVersion : 1\r\n", "cr.pkg.tar.zst": "Version : 1\rName : yggdrasil-cr\r",
           "spaces.pkg.tar.zst": "Name : yggdrasil-sp x\n", "late.pkg.tar.zst": "Name : \nName : yggdrasil-z\n",
           "indented.pkg.tar.zst": "  Name : yggdrasil-i\n", "two.pkg.tar.zst": "Other\nName:\n  heimdall-two rest\n",
           "bad.pkg.tar.zst": b"Name : yggdrasil-\xff\n"}
    each("pkg-install: pacman's text read as the script read it", [["pkg-install", PK + n] for n in sorted(odd)], pkgs=odd)
    each("pkg-install: paths", [["pkg-install"], ["pkg-install", "relative-1-1-x86_64.pkg.tar.zst"],
                                ["pkg-install", PK + "../pkgs/yggdrasil-a-1-1-x86_64.pkg.tar.zst"],
                                ["pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.gz"],
                                ["pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst\n"],
                                ["pkg-install", PK + "link.pkg.tar.zst"], ["pkg-install", PK + "dir.pkg.tar.zst"],
                                ["pkg-install", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst", PK + "openssh-1-1-x86_64.pkg.tar.zst"],
                                ["pkg-install", PK + "./yggdrasil-a-1-1-x86_64.pkg.tar.zst"], ["pkg-install", "--overwrite=*"],
                                ["pkg-install", PK + "missing.pkg.tar.zst"]],
         extra=lambda: (os.symlink("yggdrasil-a-1-1-x86_64.pkg.tar.zst", os.path.join(W, "pkgs", "link.pkg.tar.zst")),
                        os.makedirs(os.path.join(W, "pkgs", "dir.pkg.tar.zst"))))
    odd_names = {"sp ace\udcff.pkg.tar.zst": pkg("yggdrasil-odd")}
    one("pkg-install: a file name with a space and a byte that is not UTF-8", "pkg-install", PK + "sp ace\udcff.pkg.tar.zst", pkgs=odd_names)

    # -- pkg-add
    one("pkg-add: one", "pkg-add", "htop")
    one("pkg-add: two", "pkg-add", "htop", "gnu-efi")
    one("pkg-add: pacman fails", "pkg-add", "htop", rc={"pacman": 1})
    each("pkg-add: not in the repositories", [["pkg-add", "nothere"], ["pkg-add", "htop", "nothere", "gnu-efi"]])
    each("pkg-add: remote-access tools", [["pkg-add", n] for n in ("tailscale", "cloudflared-bin", "zerotier-one", "netbird-bin", "frps", "tor",
                                                                   "xorg-server", "python-lsp-server", "gnome-remote-desktop", "openssh",
                                                                   "anydesk-bin", "ttyd", "cockpit", "headscale", "bore-cli")])
    each("pkg-add: a remote-access tool among what it brings", [["pkg-add", "sshfs"], ["pkg-add", "htop", "remmina"], ["pkg-add", "python-lsp"]])
    one("pkg-add: pacman -Sp fails", "pkg-add", "htop", sp_fails=True)
    each("pkg-add: not package names", [["pkg-add"], ["pkg-add", "--overwrite=*"], ["pkg-add", "-x"], ["pkg-add", "a b"], ["pkg-add", "Htop"],
                                        ["pkg-add", "extra/htop"], ["pkg-add", "h" * 101], ["pkg-add", "htop\n"], ["pkg-add", "é"],
                                        ["pkg-add", "it's"], ["pkg-add", "x\udcff"], ["pkg-add", "​"], ["pkg-add", ""],
                                        ["pkg-add", "htop", "-u"], ["pkg-add", "htop>=1"]])

    # -- service
    each("service: every action", [["service", a, "yggdrasil-tune.service"] for a in
                                   ("start", "stop", "restart", "reload", "status", "is-active", "enable", "disable", "enable-now",
                                    "disable-now", "user-enable", "user-disable")] + [["service", "daemon-reload"]])
    each("service: units", [["service", "restart", u] for u in
                            ("heimdall", "heimdall.service", "limbo.service", "sarina-service", "ollama", "systemd-sysctl.service",
                             "greetd-test", "yggdrasil-x@.service", "labyrinth-x@foo:bar.service", "labyrinth-voiced.service")]
         + [["service", "restart", "heimdall", "limbo", "yggdrasil-a.timer"]])
    each("service: not ours", [["service", "restart", u] for u in
                               ("sshd", "ufw.service", "heimdall;id", "/tmp/x.service", "-heimdall", "--now", "labyrinth-x/../y",
                                "yggdrasil-x\n", "yggdrasil-é", "", "yggdrasil", "labyrinth", "Heimdall")]
         + [["service", "restart", "heimdall", "sshd"], ["service", "restart"], ["service", "daemon-reload", "heimdall"]])
    each("service: the agent window's units", [["service", a, "labyrinth-agent-window-close.timer"] for a in ("stop", "restart", "start", "status")]
         + [["service", "stop", "labyrinth-agent"], ["service", "restart", "yggdrasil-x", "labyrinth-agent-window-close.service"]])
    each("service: Heimdall is not stopped", [["service", a, u] for a in ("stop", "disable", "disable-now", "user-disable", "restart", "enable-now")
                                               for u in ("heimdall.service", "heimdall-scan.timer")])
    each("service: actions not allowed", [["service", a, "heimdall"] for a in ("mask", "edit", "link", "kill", "set-property", "isolate",
                                                                               "START", "--now", "")])
    one("service: systemctl's exit code", "service", "is-active", "limbo", rc={"systemctl": 3})
    one("service: systemctl killed", "service", "restart", "limbo", rc={"systemctl": "TERM"})

    # -- logs
    each("logs: read-only", [["logs"], ["logs", "-u", "heimdall", "-n", "50", "--since", "-1h"], ["logs", "-b", "-1", "-k"],
                             ["logs", "_COMM=sudo", "--output=cat"], ["logs", "-f", "-r", "-x", "-a", "-q", "--no-hostname", "--case-sensitive"],
                             ["logs", "--dmesg", "--follow", "--reverse", "--catalog", "--all", "--quiet"], ["logs", "-t", "sudo", "-p", "err"],
                             ["logs", "-g", "x.*y", "--grep", "--rotate"], ["logs", "--unit=heimdall", "--lines=5", "-n=5", "-u=x"],
                             ["logs", "-o", "json", "--until", "today"], ["logs", "_SYSTEMD_UNIT=a/b", "_PID=1", "PRIORITY=3", "_COMM="]])
    each("logs: -n takes a count or nothing", [["logs", "-n", v] for v in ("5", "+5", "all", "05", "-k")] + [["logs", "-n"], ["logs", "--lines", "20", "-q"]])
    each("logs: -n before a writing option (the hole)", [["logs", n, w] for n in ("-n", "--lines") for w in
                                                          ("--vacuum-time=1s", "--rotate", "--flush", "--setup-keys", "--cursor-file=/etc/x",
                                                           "--vacuum-size=1", "--relinquish-var", "--update-catalog", "-D", "--directory=/tmp")])
    each("logs: -b", [["logs", "-b"], ["logs", "-b", "0"], ["logs", "--boot", "-3"], ["logs", "-b", "0123456789abcdef0123456789abcdef"],
                      ["logs", "-b", "0123456789ABCDEF0123456789ABCDEF"], ["logs", "-b", "5\n"], ["logs", "--boot=-1"], ["logs", "-b", "-k"],
                      ["logs", "-b", "--rotate"]])
    each("logs: writing options", [["logs", w] for w in ("--vacuum-time=1s", "--vacuum-size=1M", "--vacuum-files=1", "--rotate", "--flush",
                                                          "--sync", "--setup-keys", "--relinquish-var", "--smart-relinquish-var", "--update-catalog",
                                                          "--cursor-file=/etc/x", "-D", "--directory=/tmp", "--file=/x", "--root=/", "-M",
                                                          "--machine=x", "--", "--vac", "--rot", "-kr", "-n5", "MESSAGE=x", "_PID=1\n", "-")]
         + [["logs", "--file", "/x"], ["logs", "-D", "/tmp/x"], ["logs", "-u"], ["logs", "-k", "--grep"]])
    each("logs: a value that looks like an option is a value", [["logs", "-u", "--vacuum-time=1s"], ["logs", "--since", "--rotate"],
                                                                 ["logs", "-g", "--flush"]])
    one("logs: journalctl's exit code", "logs", "-k", rc={"journalctl": 1})

    # -- maintenance
    each("maintenance", [["maintenance", "on", "--for", "2h", "--reason", "why"], ["maintenance", "off"], ["maintenance", "status"],
                         ["maintenance", "on"], ["maintenance", "status", "--for", "x", "--reason", "y"], ["maintenance", "on", "--reason", "a b"],
                         ["maintenance", "on", "--for", "2h", "--for", "3h"], ["maintenance", "on", "--for", "--config"],
                         ["maintenance", "on", "--for", "2h", "--reason", "--config=/tmp/x"]])
    each("maintenance: shapes refused", [["maintenance", "on", "--for"], ["maintenance", "on", "--for", "2h", "--reason"],
                                         ["maintenance", "on", "--config", "/x"], ["maintenance", "on", "--reason", "a", "--config", "b"],
                                         ["maintenance", "on", "x", "y"], ["maintenance", "on", "--for=2h"], ["maintenance", "on", "-for", "2h"],
                                         ["maintenance", "on", "--for", "2h", "--reason", "r", "x"], ["maintenance", "ON"]])
    one("maintenance: heimdalld's exit code", "maintenance", "on", "--for", "9h", "--reason", "x", rc={"heimdalld": 2})

    # -- limbo
    each("limbo", [["limbo", "up", "/var/lib/limbo/build-agent/t.json"], ["limbo", "down", "/etc/limbo/validation/a.json"], ["limbo", "doctor"],
                   ["limbo", "status"], ["limbo", "doctor", "/etc/limbo/validation/a.json"], ["limbo", "up", "/var/lib/limbo/build-agent/t.json", "--i-know"],
                   ["limbo", "status", ""]])
    each("limbo: refused", [["limbo", "up"], ["limbo", "up", "/home/alex/evil.json"], ["limbo", "up", "/var/lib/limbo/build-agent/../../../etc/x.json"],
                            ["limbo", "up", "/var/lib/limbo/build-agent/t.yaml"], ["limbo", "up", "/var/lib/limbo/build-agent/t.json\n"],
                            ["limbo", "hold"], ["limbo", "status", "/etc/shadow"], ["limbo", "doctor", "--state-dir=/tmp/x"],
                            ["limbo", "status", "/var/lib/limbo/build-agent-x/t.json"], ["limbo", "UP", "/etc/limbo/validation/a.json"]])

    # -- rollback, firewall
    each("rollback", [["rollback", "list"], ["rollback", "status"], ["rollback", "dry-run", "12"], ["rollback", "dry-run", "²"],
                      ["rollback", "dry-run", "١٢"], ["rollback", "dry-run", "1a"], ["rollback", "dry-run"], ["rollback", "list", "x"],
                      ["rollback", "3"], ["rollback", "undo"], ["rollback", "dry-run", "-1"], ["rollback", "dry-run", ""]])
    each("firewall-status", [["firewall-status"], ["firewall-status", "--force", "x"]])

    # -- deploy
    each("deploy: each place", [["deploy", SRC + "exe", "/usr/bin/yggdrasil-new"], ["deploy", SRC + "pub", "/usr/bin/minos"],
                                ["deploy", SRC + "pub", "/usr/bin/ygg-x"], ["deploy", SRC + "pub", "/usr/share/yggdrasil/new/sub/x.png"],
                                ["deploy", SRC + "pub", "/usr/share/labyrinth/a.txt"], ["deploy", SRC + "pub", "/usr/share/icons/hicolor/x.svg"],
                                ["deploy", SRC + "pub", "/usr/share/applications/x.desktop"],
                                ["deploy", SRC + "pub", "/usr/share/wayland-sessions/y.desktop"], ["deploy", SRC + "pub", "/etc/yggdrasil/a.json"],
                                ["deploy", SRC + "pub", "/etc/yggdrasil/b.conf"]])
    one("deploy: over what is there", "deploy", SRC + "exe", "/usr/share/yggdrasil/old.png",
        existing={"/usr/share/yggdrasil/old.png": ("old\n", 0o600)})
    one("deploy: /etc/yggdrasil made", "deploy", SRC + "pub", "/etc/yggdrasil/new.json", etc_yggdrasil=False)
    each("deploy: places refused", [["deploy", SRC + "pub", d] for d in
                                    ("/usr/lib/yggdrasil/tune", "/usr/local/bin/labyrinth-x", "/etc/sudoers.d/x", "/usr/bin/sudo",
                                     "/etc/systemd/system/x.service", "/usr/share/yggdrasil/../../../etc/passwd", "/usr/share/polkit-1/actions/x.policy",
                                     "/etc/yggdrasil/../sudoers", "/usr/share/yggdrasil/./x", "/usr/share/yggdrasil/x/", "/usr/share/yggdrasil//x",
                                     "/usr/share/yggdrasil/.", "/usr/bin/yggdrasil-x\n", "/etc/yggdrasil/x.json\n", "/usr/bin/yggdrasil-",
                                     "/usr/bin/yggdrasil-X", "/usr/share/icons/.svg", "/usr/share/applications/a/b.desktop", "relative/x",
                                     "", "/usr/share/yggdrasil/\udcff", "/etc/yggdrasil/x.yaml", "/usr/bin/ygg-x/y")])
    each("deploy: where root looks", [["deploy", SRC + "pub", d] for d in
                                      ("/usr/share/yggdrasil/gpu-unlock/supported.json", "/usr/share/yggdrasil/gpu-unlock",
                                       "/usr/share/labyrinth/pkgbuilds/yggdrasil-x/PKGBUILD", "/usr/share/labyrinth/grub-evodevo/install.sh")]
         + [["deploy", SRC + "pub", "/usr/share/yggdrasil/gpu-unlocked.json"]])
    each("deploy: programs root runs", [["deploy", SRC + "exe", "/usr/bin/" + n] for n in
                                        ("yggdrasil-spell", "yggdrasil-gametime", "yggdrasil-hooked", "yggdrasil-polkit", "yggdrasil-sudo",
                                         "yggdrasil-udev", "yggdrasil-dropin", "yggdrasil-transient", "yggdrasil-local", "yggdrasil-spellcheck",
                                         "yggdrasil-x")],
         units=dict(STANDARD_UNITS, **{
             "/usr/share/libalpm/hooks/90-x.hook": "[Action]\nExec = /usr/bin/yggdrasil-hooked --sync\n",
             "/usr/share/polkit-1/actions/org.x.policy": '<annotate key="org.freedesktop.policykit.exec.path">/usr/bin/yggdrasil-polkit</annotate>\n',
             "/etc/sudoers.d/60-x": "sarina ALL=(root) NOPASSWD: /usr/bin/yggdrasil-sudo\n",
             "/usr/lib/udev/rules.d/99-x.rules": 'ACTION=="add", RUN+="/usr/bin/yggdrasil-udev"\n',
             "/etc/systemd/system/z.service.d/override.conf": "[Service]\nExecStart=\nExecStart=/usr/bin/yggdrasil-dropin\n",
             "/usr/local/lib/systemd/system/l.service": "[Service]\nExecStart=yggdrasil-local\n",
             "/etc/pacman.d/hooks/deeper/x/y.hook": "Exec = /usr/bin/yggdrasil-x\n"}),
         run_units={"run-u.service": "[Service]\nExecStart=/usr/bin/yggdrasil-transient\n"},
         links={"/etc/systemd/system/masked.service": "/dev/null"})
    each("deploy: the source, read as the caller", [["deploy", SRC + "pub", "/usr/share/yggdrasil/a.txt"],
                                                    ["deploy", SRC + "secret", "/usr/share/yggdrasil/b.txt"],
                                                    ["deploy", SRC + "missing", "/usr/share/yggdrasil/c.txt"],
                                                    ["deploy", SRC + "link", "/usr/share/yggdrasil/d.txt"],
                                                    ["deploy", SRC + "dir", "/usr/share/yggdrasil/e.txt"],
                                                    ["deploy", "{WORLD}/ov/fifo", "/usr/share/yggdrasil/f.txt"],
                                                    ["deploy", SRC + "closed/pub", "/usr/share/yggdrasil/g.txt"],
                                                    ["deploy", "/etc/shadow", "/usr/share/yggdrasil/shadow.txt"],
                                                    ["deploy", "relative", "/usr/share/yggdrasil/h.txt"]],
         extra=lambda: (os.symlink("pub", os.path.join(W, "src", "link")), os.makedirs(os.path.join(W, "src", "dir")),
                        os.mkfifo(os.path.join(W, "ov", "fifo")), P.write(os.path.join(W, "src", "closed", "pub"), "x", 0o644),
                        os.chmod(os.path.join(W, "src", "closed"), 0o700)))
    case("deploy: root's own sources when sudo names no caller", [{"args": ["deploy", SRC + "secret", "/usr/share/yggdrasil/b.txt"],
                                                                    "env": {"SUDO_UID": None, "SUDO_GID": None}},
                                                                   {"args": ["deploy", SRC + "secret", "/usr/share/yggdrasil/c.txt"],
                                                                    "env": {"SUDO_UID": "0", "SUDO_GID": "0"}}])
    case("deploy: SUDO_UID and SUDO_GID that name no account", [{"args": ["deploy", SRC + "pub", "/usr/share/yggdrasil/x.txt"], "env": e}
                                                                for e in ({"SUDO_UID": ""}, {"SUDO_GID": None}, {"SUDO_UID": None},
                                                                          {"SUDO_UID": "x"}, {"SUDO_UID": "4294967295"}, {"SUDO_UID": "12345678901"},
                                                                          {"SUDO_UID": "+5"}, {"SUDO_UID": "٣"})])
    each("deploy: links on the way", [["deploy", SRC + "pub", "/usr/share/icons/theme/x.png"],
                                      ["deploy", SRC + "pub", "/usr/share/yggdrasil/a/b/c/x.png"],
                                      ["deploy", SRC + "pub", "/usr/share/yggdrasil/a/x.png"],
                                      ["deploy", SRC + "pub", "/usr/share/applications/l.desktop"]],
         links={"/usr/share/icons/theme": "/etc", "/usr/share/yggdrasil/a": "/etc/yggdrasil", "/usr/share/applications/l.desktop": "/etc/passwd"},
         existing={"/etc/yggdrasil/b/c/keep.conf": "kept\n"})
    one("deploy: a stale .agent-tmp", "deploy", SRC + "pub", "/usr/share/yggdrasil/s.txt",
        existing={"/usr/share/yggdrasil/s.txt.agent-tmp": ("half\n", 0o644)})
    one("deploy: over a folder", "deploy", SRC + "pub", "/usr/share/yggdrasil/folder",
        existing={"/usr/share/yggdrasil/folder/inside": "x\n"})
    one("deploy: a file on the way", "deploy", SRC + "pub", "/usr/share/yggdrasil/f/x.png", existing={"/usr/share/yggdrasil/f": "x\n"})
    case("deploy: a strict umask", [{"args": ["deploy", SRC + "pub", "/usr/share/yggdrasil/u/v/w.png"],
                                     "wrap": WRAP + ["/bin/sh", "-c", 'umask 077; exec "$@"', "sh"]}])

    # -- the environment the caller brings, and long lines
    case("the caller's environment does not reach the programs",
         [{"args": a, "env": {"PATH": "{WORLD}/evil:/usr/bin:/bin", "LD_PRELOAD": "{WORLD}/nothing.so", "SYSTEMD_PAGER": "sh", "PAGER": "sh",
                              "LIMBO_GO_BINARY": "{WORLD}/evil/pacman", "LIMBO_STATE_DIR": "/tmp", "HOME": "{WORLD}/home", "LESSOPEN": "|x",
                              "SUDO_HOME": "/home/claude-agent", "TERM": "../../x", "LANG": "de_DE.UTF-8", "SYSTEMD_LOG_LEVEL": "debug"}}
          for a in (["pkg-add", "htop"], ["service", "status", "heimdall"], ["logs", "-k"], ["limbo", "doctor"],
                    ["maintenance", "status"], ["firewall-status"], ["rollback", "list"], ["foo"])],
         extra=lambda: P.write(os.path.join(W, "evil", "pacman"), "#!/bin/sh\necho EVIL >> %s/calls\n" % W, 0o755))
    each("long arguments: the journal line is cut", [["service", "restart"] + ["x" * 60000] * 3, ["logs", "y" * 100000],
                                                     ["pkg-add", "z" * 50000, "w"], ["service", "status", "limbo"] * 1])
    each("bytes that are not UTF-8", [["service", "restart", "yggdrasil-\udcff"], ["logs", "_COMM=\udcc3"], ["limbo", "up", "/x\udcff.json"],
                                      ["deploy", SRC + "pub", "/usr/share/yggdrasil/\udce9t\udcff"], ["\udcff"], ["pkg-add", "\udcc3\udca9"]])

    # -- random command lines (the same in both, every call within the allow-list)
    VOCAB = ["pkg-install", "pkg-add", "service", "logs", "maintenance", "limbo", "rollback", "firewall-status", "deploy", "--", "", "on", "off",
             "status", "--for", "--reason", "--config", "4h", "up", "down", "doctor", "list", "dry-run", "12", "-1", "+5", "all", "start", "stop",
             "restart", "enable-now", "disable", "user-disable", "daemon-reload", "mask", "heimdall", "heimdall.service",
             "labyrinth-agent-window-close.timer", "yggdrasil-tune", "limbo.service", "sshd", "-u", "-n", "--lines", "-b", "-o", "-g", "-k", "-f",
             "--case-sensitive", "--vacuum-time=1s", "--rotate", "--flush", "--setup-keys", "--cursor-file=/etc/x", "-D", "--file=/x", "-kr",
             "_COMM=sudo", "MESSAGE=x", "0123456789abcdef0123456789abcdef", "htop", "sshfs", "remmina", "tailscale", "openssh", "xorg-server",
             "--overwrite=*", PK + "yggdrasil-a-1-1-x86_64.pkg.tar.zst", PK + "openssh-1-1-x86_64.pkg.tar.zst", PK + "sudo-1-1-x86_64.pkg.tar.zst",
             PK + "noname-1-1-x86_64.pkg.tar.zst", PK + "missing.pkg.tar.zst", "/var/lib/limbo/build-agent/t.json", "/etc/shadow",
             "/etc/limbo/validation/a.json", SRC + "pub", SRC + "secret", SRC + "exe", "/usr/bin/yggdrasil-x", "/usr/bin/yggdrasil-spell",
             "/usr/bin/minos", "/usr/share/yggdrasil/r/x.png", "/usr/share/yggdrasil/gpu-unlock/supported.json", "/usr/share/icons/t/a.svg",
             "/etc/yggdrasil/r.json", "/etc/sudoers.d/x", "/usr/share/applications/r.desktop", "\udcff", "x\n"]
    rng = random.Random(20261111)
    for k in range(int(os.environ.get("LABYRINTH_AGENT_RANDOM", "400"))):
        words = [rng.choice(VOCAB[:9]) if rng.random() < 0.85 else rng.choice(VOCAB)]
        words += [rng.choice(VOCAB) for _ in range(rng.randrange(0, 5))]
        env = {} if rng.random() < 0.7 else {"SUDO_UID": None, "SUDO_GID": None}
        one("random %03d: %s" % (k, " ".join(w.replace("{WORLD}", "W") for w in words)[:90]), *words, env=env)
