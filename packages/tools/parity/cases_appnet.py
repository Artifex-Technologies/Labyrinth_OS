"""The cases of labyrinth-appnet, labyrinth-appnet-ask, labyrinth-appnet-ask-agent and labyrinth-launch (loaded by compare.py).
nft, logger, systemctl, systemd-run, notify-send and the kernel log are stand-ins; nothing reaches the real firewall. `set` runs
as "root" of a user namespace of its own (unshare -r)."""
import json
import os
import re
import socket
import time

import __main__ as P

W = P.WORLD
HOME = os.path.join(W, "home")
UID = str(os.getuid())
STATE = os.path.join(W, "etc", "app-network.json")
ASK = os.path.join(W, "var", "app-ask.json")
CG = os.path.join(W, "cgroup")
AN = "labyrinth-appnet"
ENV = {"LABYRINTH_APPNET_STATE": STATE, "LABYRINTH_ASK_STATE": ASK, "LABYRINTH_CGROUP_ROOT": CG, "STDIN_nft": "-f"}
ROOT = ["unshare", "-r"]


def cg(uid, app):
    return os.path.join(CG, "user.slice/user-%s.slice/user@%s.service/lapp_%s.slice" % (uid, uid, re.sub(r"[^A-Za-z0-9_]", "_", app)))


# a PATH without the system's folders, so no real systemctl, systemd-run, nft or bwrap is ever found: the stand-ins in
# WORLD/bin, and in WORLD/pybin what the scripts and the stand-ins themselves need
MINI = os.path.join(W, "bin") + ":" + os.path.join(W, "pybin")


def pybin():
    os.makedirs(os.path.join(W, "pybin"), exist_ok=True)
    for t in ("python3", "basename", "tr", "cat", "sleep", "timeout"):
        if not os.path.lexists(os.path.join(W, "pybin", t)):
            os.symlink("/usr/bin/" + t, os.path.join(W, "pybin", t))


def world(files=(), slices=(), tools=("nft", "logger", "systemctl")):
    def go(paths):
        pybin()
        P.stubs(tools)
        for p, body, *mode in files:
            P.write(p, body if isinstance(body, (str, bytes)) else json.dumps(body), mode[0] if mode else 0o644)
        for uid, app in slices:
            os.makedirs(cg(uid, app), exist_ok=True)
    return go


def an(name, steps, files=(), slices=(), env=None, **k):
    e = dict(ENV)
    e.update(env or {})
    P.case(AN, name, steps, setup=world(files, slices), env=e, **k)


RULES = {"rules": {UID: {"firefox": "block", "spotify-launcher": "wifi", "steam": "ethernet", "pdfcraft": "ask", "com.x.y": "allow"},
                   "1001": {"other": "block"}}}
ASKED = {UID: {"pdfcraft": {"allow": {"1.2.3.4": "a.example", "2606:4700::1111": "", "9.9.9.9": 5, "bad ip": "x"}, "deny": {"5.6.7.8": "d"}},
               "../bad": {"allow": {}}, "steam": "notadict"}}
ALL_UP = [(UID, a) for a in RULES["rules"][UID]] + [("1001", "other")]

for args in [[], ["-h"], ["--help"], ["nope"], ["set", "a"], ["set", "a", "b", "c"]]:
    an("arguments: %r" % args, [{"args": args}])
an("get and ruleset: every mode, every slice running", [{"args": ["get"]}, {"args": ["ruleset"]}], files=[(STATE, RULES), (ASK, ASKED)], slices=ALL_UP)
an("ruleset: slices not running are left out", [{"args": ["ruleset"]}], files=[(STATE, RULES)], slices=[(UID, "steam")])
an("ruleset: no rules file, no ask file", [{"args": ["ruleset"]}, {"args": ["get"]}])
for label, content in [("not JSON", "{"), ("not UTF-8", b'{"rules": {"1000": {"a\xff": "block"}}}'), ("a list", "[1]"), ("rules missing", '{"x": 1}'),
                       ("rules a list", '{"rules": [1]}'), ("rules text", '{"rules": "abc"}'), ("rules null", '{"rules": null}'),
                       ("odd entries", {"rules": {UID: {"ok": "block", "../x": "block", "bad": "explode", "free": "allow", ".hidden": "block", "n": 5}, "x": 5, "y": []}}),
                       ("uids that int() reads", {"rules": {" 1000": {"a": "block"}, "1_000": {"b": "wifi"}, "١٠": {"c": "block"}, "1000": {"d": "block"}}}),
                       ("a uid int() refuses", {"rules": {"abc": {"a": "block"}}}),
                       ("long and odd app ids", {"rules": {UID: {"a" * 120: "block", "a" * 121: "block", "x+y.z_w-v": "wifi"}}})]:
    an("state file: " + label, [{"args": ["get"]}, {"args": ["ruleset"]}], files=[(STATE, content)], slices=[(UID, "ok"), (UID, "a"), ("1000", "a")])
for label, content in [("allow a list", {UID: {"pdfcraft": {"allow": [1]}}}), ("allow text", {UID: {"pdfcraft": {"allow": "1.2.3.4"}}}),
                       ("allow empty text", {UID: {"pdfcraft": {"allow": "", "deny": 0}}}), ("top a list", [1]), ("not JSON", "x"),
                       ("odd addresses", {UID: {"pdfcraft": {"allow": {"01.2.3.4": "", "fe80::1%eth0": "", "::ffff:1.2.3.4": "", "1.2.3.4/32": "", "::": "z"}}}})]:
    an("ask file: " + label, [{"args": ["ruleset"]}], files=[(STATE, RULES), (ASK, content)], slices=[(UID, "pdfcraft")])
an("apply: loaded", [{"args": ["apply"]}, {"args": ["apply", "extra"]}], files=[(STATE, RULES), (ASK, ASKED)], slices=ALL_UP)
an("apply: nft refuses, says why", [{"args": ["apply"]}], files=[(STATE, RULES)], slices=ALL_UP, env={"RC_nft": "1", "ERR_nft": "\nError: syntax\nline 2\n"})
an("apply: nft refuses silently", [{"args": ["apply"]}], files=[(STATE, RULES)], env={"RC_nft": "3"})
an("apply: nft is missing", [{"args": ["apply"]}], files=[(STATE, RULES)], env={"PATH": os.path.join(W, "pybin")})
an("set: not root", [{"args": ["set", "firefox", "block"]}], files=[(STATE, RULES)])
an("set: as root, for PKEXEC_UID", [{"args": ["set", "firefox", "wifi"], "wrap": ROOT}, {"args": ["set", "new.app", "ask"], "wrap": ROOT},
                                    {"args": ["set", "steam", "allow"], "wrap": ROOT}, {"args": ["get"]}],
   files=[(STATE, RULES)], slices=ALL_UP + [(UID, "new.app")], env={"PKEXEC_UID": UID})
an("set: the last app of a person goes, then the person", [{"args": ["set", "other", "allow"], "wrap": ROOT}, {"args": ["get"]}],
   files=[(STATE, RULES)], env={"PKEXEC_UID": "1001"})
an("set: a new file, its folder made", [{"args": ["set", "a", "block"], "wrap": ROOT}], env={"PKEXEC_UID": "7", "LABYRINTH_APPNET_STATE": os.path.join(W, "new", "dir", "s.json")})
an("set: bad app or mode", [{"args": ["set", "a b", "block"], "wrap": ROOT}, {"args": ["set", "a", "explode"], "wrap": ROOT}, {"args": ["set", "", "block"], "wrap": ROOT}],
   env={"PKEXEC_UID": UID})
an("set: who asked", [{"args": ["set", "a", "block"], "wrap": ROOT, "env": {"PKEXEC_UID": None}},
                      {"args": ["set", "a", "block"], "wrap": ROOT, "env": {"PKEXEC_UID": "", "SUDO_UID": "12"}},
                      {"args": ["set", "a", "block"], "wrap": ROOT, "env": {"PKEXEC_UID": "x1"}},
                      {"args": ["set", "a", "block"], "wrap": ROOT, "env": {"PKEXEC_UID": "", "SUDO_UID": ""}},
                      {"args": ["get"]}])
an("set: a uid that isdigit() takes and int() refuses", [{"args": ["set", "a", "block"], "wrap": ROOT}], env={"PKEXEC_UID": "²"})
an("set: nft refuses", [{"args": ["set", "firefox", "block"], "wrap": ROOT}], files=[(STATE, RULES)], slices=ALL_UP,
   env={"PKEXEC_UID": UID, "RC_nft": "1", "ERR_nft": "Error: no\n"})
an("prepare: the slices of my apps, then the service", [{"args": ["prepare"]}], files=[(STATE, RULES)])
an("prepare: no rules of mine", [{"args": ["prepare"]}], files=[(STATE, {"rules": {"1001": {"a": "block"}}})])
an("prepare: systemctl is missing", [{"args": ["prepare"]}], files=[(STATE, RULES)], env={"PATH": os.path.join(W, "pybin")})

# ---- labyrinth-appnet-ask: the service, a fake kernel log, a client that asks and answers

KLOG = r"""#!/bin/sh
# a fake `journalctl -k -f`: the lines of klog.txt; "WAIT name" waits until WORLD/name exists
while IFS= read -r l; do
  case "$l" in
    "WAIT "*) n=${l#WAIT }; while [ ! -e "$WORLD/$n" ]; do sleep 0.02; done ;;
    *) printf '%s\n' "$l"; sleep 0.05 ;;
  esac
done < "$WORLD/klog.txt"
exec sleep 1000
"""


def kline(app, dst, dpt=443, uid=UID, proto="TCP"):
    return "labask:%s IN= OUT=wlan0 SRC=192.168.1.5 DST=%s LEN=60 PROTO=%s SPT=40000 DPT=%d WINDOW=1 UID=%s GID=%s" % (app, dst, proto, dpt, uid, uid)


class Client:
    def __init__(self, path, log, name):
        self.s = None
        for _ in range(200):
            try:
                self.s = socket.socket(socket.AF_UNIX)
                self.s.connect(path)
                break
            except OSError:
                time.sleep(0.02)
        self.buf = b""
        self.log, self.name = log, name

    def send(self, line):
        self.log.append("%s> %s" % (self.name, line))
        try:
            self.s.sendall((line if isinstance(line, bytes) else line.encode()) + b"\n")
        except OSError as e:
            self.log.append("%s! send: %s" % (self.name, type(e).__name__))

    def recv(self, n=1, wait=2.0):
        """n lines, each waited for up to `wait` seconds ("(nothing)" when none came, "(end)" when the service closed)."""
        import select
        for _ in range(n):
            end = time.time() + wait
            while b"\n" not in self.buf:
                left = end - time.time()
                if left <= 0 or not select.select([self.s], [], [], left)[0]:
                    self.log.append("%s< (nothing)" % self.name)
                    break
                try:
                    chunk = self.s.recv(65536)
                except OSError as e:
                    chunk = b""
                    self.log.append("%s< (%s)" % (self.name, type(e).__name__))
                if not chunk:
                    self.log.append("%s< (end)" % self.name)
                    return
                self.buf += chunk
            else:
                line, _, self.buf = self.buf.partition(b"\n")
                self.log.append("%s< %s" % (self.name, line.decode("utf-8", "replace")))


def ask_conversation(paths):
    sock = os.path.join(W, "run", "labyrinth", "ask.sock")
    log = []
    a = Client(sock, log, "A")
    a.send('{"op":"subscribe"}')
    b = Client(sock, log, "B")
    b.send('{"op":"list"}')
    b.recv()
    time.sleep(0.2)
    open(os.path.join(W, "go1"), "w").close()
    a.recv(2)                                   # two questions (the duplicate and the other apps are not asked)
    a.recv(1, wait=0.6)                         # and no third
    a.send('{"op":"decide","id":1,"verdict":"allow","name":"ex ample.com/<x>"}')
    a.recv()
    a.send('{"op":"decide","id":2.0,"verdict":"deny"}')
    a.recv()
    a.send('{"op":"decide","id":99,"verdict":"allow"}')
    a.recv()
    a.send('{"op":"decide","id":3,"verdict":"bogus"}')
    a.recv()
    a.send('not json')
    a.send('{"op":"nothing"}')
    a.send(b'\xff\xfe')
    a.send('{"op":"list"}')
    a.recv()
    a.send('{"op":"forget","app":"pdfcraft","ip":"9.9.9.9"}')
    a.recv()
    a.send('{"op":"forget","app":"pdfcraft","ip":"not an ip"}')
    a.recv()
    a.send('{"op":"forget"}')
    a.recv()
    open(os.path.join(W, "go2"), "w").close()
    a.recv(2)                                   # 93.184.216.34 is decided: only the new ones (3 and 4)
    a.send('{"op":"decide","id":4,"verdict":"ignore"}')
    a.recv()
    c = Client(sock, log, "C")
    c.send('{"op":"subscribe"}')
    c.recv()                                    # the pending question 3 goes to every subscriber of this person: C
    a.recv()                                    # and A again
    a.send('{"op":"decide","id":true,"verdict":"allow"}')
    a.recv()                                    # true is the id 1, decided already
    time.sleep(1.3)                             # the cooldown (1 s) of 10.0.0.2 is over
    open(os.path.join(W, "go3"), "w").close()
    a.recv()                                    # question 5
    c.recv()
    a.send('{"op":"decide","id":5,"verdict":"allow-all","name":5}')
    a.recv()                                    # a name that is not text: the script's handler stopped, the connection ends
    d = Client(sock, log, "D")
    d.send('{"op":"decide","id":5,"verdict":"allow-all"}')
    d.recv()                                    # still pending: viewer goes back to any connection, the rules loaded again
    d.send('[1, 2]')
    d.recv()                                    # not an object: the connection ends
    e = Client(sock, log, "E")
    e.send('{"op":"list"}')
    e.recv()
    P.write(os.path.join(W, "transcript"), "\n".join(log) + "\n")


ASK_RULES = {"rules": {UID: {"pdfcraft": "ask", "other": "block", "viewer": "ask"}}}
P.case("labyrinth-appnet-ask", "the service: questions, answers, lists, forgets, cooldown, bad requests",
       [{}], setup=lambda paths: (world([(STATE, ASK_RULES), (ASK, {UID: {"pdfcraft": {"allow": {"9.9.9.9": "n"}, "deny": {}}}}),
                                          (os.path.join(W, "bin", "klog"), KLOG, 0o755),
                                          (os.path.join(W, "klog.txt"), "\n".join([
                                              "WAIT go1", kline("pdfcraft", "93.184.216.34"), kline("pdfcraft", "93.184.216.34", 80), kline("other", "1.1.1.1"),
                                              "some other kernel line", kline("pdfcraft", "2606:4700::1111", 53, proto="UDP"), kline("ghost", "1.1.1.1"),
                                              kline("pdfcraft", "8.8.8.8", uid="0"),
                                              "WAIT go2", kline("pdfcraft", "93.184.216.34"), kline("viewer", "10.0.0.1"), kline("viewer", "10.0.0.2"),
                                              "WAIT go3", kline("viewer", "10.0.0.1"), kline("viewer", "10.0.0.2")]) + "\n")],
                                         slices=[(UID, "pdfcraft"), (UID, "viewer")])(paths)),
       env=dict(ENV, LABYRINTH_ASK_SOCKET=os.path.join(W, "run", "labyrinth", "ask.sock"), LABYRINTH_NFT=os.path.join(W, "bin", "nft"),
                LABYRINTH_ASK_LOGCMD=os.path.join(W, "bin", "klog"), LABYRINTH_ASK_COOLDOWN="1"),
       watch=ask_conversation, differs={("err", 0)})
P.case("labyrinth-appnet-ask", "the service: --help and arguments", [{"args": ["--help"]}, {"args": ["x", "y"]}])
P.case("labyrinth-appnet-ask", "the service: a cooldown that is not a number", [{}], env={"LABYRINTH_ASK_COOLDOWN": "soon"})


def missing_log(paths):
    time.sleep(1.0)


P.case("labyrinth-appnet-ask", "the service: the kernel log command is missing", [{}], setup=world(),
       env=dict(ENV, LABYRINTH_ASK_SOCKET=os.path.join(W, "s.sock"), LABYRINTH_ASK_LOGCMD="no-such-journalctl -k"), watch=missing_log)

# ---- labyrinth-appnet-ask-agent: a fake service, a fake notify-send that answers by the app's name

NOTIFY = r"""#!/bin/sh
line=notify
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
case "$*" in
  *Pdfcraft*) echo allow ;;
  *"Text Editor"*) echo all ;;
  *Viewer*) echo deny ;;
  *Slow*) sleep 3; echo allow ;;
  *) echo maybe ;;
esac
"""


def fake_service(paths):
    sock = os.path.join(W, "ask.sock")
    srv = socket.socket(socket.AF_UNIX)
    srv.bind(sock)
    srv.listen(8)
    srv.settimeout(5)
    log = []
    sub, _ = srv.accept()
    sub.settimeout(5)
    log.append("subscribe: %r" % sub.recv(100))
    qs = [{"type": "ask", "id": 1, "app": "pdfcraft", "ip": "127.0.0.1", "port": 443, "proto": "TCP"},
          {"type": "ask", "id": 2, "app": "org.gnome.text_editor", "ip": "::1", "port": 0, "proto": "UDP"},
          {"type": "ask", "id": 1, "app": "pdfcraft", "ip": "127.0.0.1", "port": 443, "proto": "TCP"},
          {"type": "list", "apps": {}},
          {"type": "ask", "id": 3, "app": "viewer", "ip": "192.0.2.1", "port": 8080, "proto": "TCP"},
          {"type": "ask", "id": 4, "app": "x.y", "ip": "192.0.2.1", "port": 1, "proto": "ICMP"}]
    sub.sendall(("\n".join(json.dumps(q) for q in qs) + "\nnot json\n").encode())
    answers = []
    end = time.time() + 6
    while len(answers) < 4 and time.time() < end:
        try:
            c, _ = srv.accept()
        except OSError:
            break
        c.settimeout(3)
        answers.append(c.recv(4096).decode())
        c.sendall(b'{"type": "result", "ok": true, "message": "ok"}\n')
        c.close()
    log += sorted(answers)
    sub.close()
    time.sleep(0.3)
    P.write(os.path.join(W, "transcript"), "\n".join(log) + "\n")
    srv.close()


P.case("labyrinth-appnet-ask-agent", "the agent: four questions (one twice), names looked up, answers sent back",
       [{}], setup=world([(os.path.join(W, "bin", "notify-send"), NOTIFY, 0o755)], tools=()),
       env={"LABYRINTH_ASK_SOCKET": os.path.join(W, "ask.sock"), "LABYRINTH_ASK_TIMEOUT": " 30"}, watch=fake_service, sort_calls=True)
P.case("labyrinth-appnet-ask-agent", "the agent: --help", [{"args": ["--help"]}])
P.case("labyrinth-appnet-ask-agent", "the agent: a timeout that is not a number", [{}], env={"LABYRINTH_ASK_TIMEOUT": "5m"})

# ---- labyrinth-launch

LA = "labyrinth-launch"
APPS = os.path.join(W, "share", "applications")
UNIT = (lambda t: re.sub(r"--unit=(lapp_[A-Za-z0-9_]*)_\d+", r"--unit=\1_<MS>", re.sub(r'(last\\?"): \d+', r'\1: <NOW>', t)))
LENV = {"XDG_DATA_DIRS": os.path.join(W, "share") + ":" + os.path.join(W, "none"), "LABYRINTH_APPNET_STATE": STATE, "XDG_RUNTIME_DIR": os.path.join(W, "run"),
        "PATH": MINI}


def desktop(name, body, where=APPS):
    return (os.path.join(where, name + ".desktop"), "[Desktop Entry]\nType=Application\n" + body)


def la(name, steps, files=(), env=None, tools=("systemd-run", "systemctl", "demo", "bwrap", "ghostty"), dirs=(), **k):
    e = dict(LENV)
    e.update(env or {})

    def go(paths):
        world(files, tools=tools)(paths)
        for d in dirs:
            os.makedirs(d, exist_ok=True)
    P.case(LA, name, steps, setup=go, env=e, settle=0.3, norm=UNIT, **k)


for args in [[], ["-h"], ["--help"], ["nothing-here"]]:
    la("arguments: %r" % args, [{"args": args}])
la("an app with files, its usage counted", [{"args": ["demo", "/a b", "/c"]}, {"args": ["demo.desktop"]}],
   files=[desktop("demo", "Name=Demo\nExec=demo --open %U %i %c\nIcon=demo\nPath=/tmp\n")])
la("field codes and quoting", [{"args": ["demo", "/x", "/y"]}],
   files=[desktop("demo", "Name=My %f App\nExec=demo \"a b\" 'c d' %f %F %u %U %d %D %n %N %v %m %k x%%y %%c --name=%c ''\nIcon=\n")])
la("an Exec that shlex cannot read", [{"args": ["demo", "/x"]}], files=[desktop("demo", "Exec=demo 'unclosed %f\n")])
la("an Exec with nothing to run", [{"args": ["demo"]}], files=[desktop("demo", "Exec=%f\n")])
la("no Exec at all", [{"args": ["demo"]}], files=[desktop("demo", "Name=x\n")])
la("a sub-folder name (vendor-app)", [{"args": ["kde-demo"]}], files=[(os.path.join(APPS, "kde", "demo.desktop"), "[Desktop Entry]\nExec=demo\n")])
la("the first folder wins; localised keys and other groups skipped", [{"args": ["demo"]}],
   files=[desktop("demo", "Name[de]=Nein\nExec = demo first \n", where=os.path.join(HOME, ".local", "share", "applications")),
          desktop("demo", "Exec=demo second\n"), (os.path.join(W, "x"), "")])
la("a terminal app", [{"args": ["demo"]}, {"args": ["demo"], "env": {"PATH": os.path.join(W, "bin2") + ":" + os.path.join(W, "pybin")}}],
   files=[desktop("demo", "Exec=demo\nTerminal=True\n"), (os.path.join(W, "bin2", "systemd-run"), P.STUB, 0o755)])
la("without systemd-run: the app alone", [{"args": ["demo", "/f"]}], files=[desktop("demo", "Exec=demo %f\n")], tools=("demo",))
la("the sandbox, files from chosen folders", [{"args": ["demo"]}],
   files=[desktop("demo", "Name=Demo\nExec=/opt/demo/bin/Demo-App --x\nStartupWMClass=DemoWin\n"),
          (os.path.join(HOME, ".config", "labyrinth", "apps", "demo.json"),
           {"sandbox": True, "files": "folders", "see_others": False, "folders": [os.path.join(HOME, "Docs"), "/etc", os.path.join(HOME, "nope")]}),
          (os.path.join(HOME, ".config", "gtk-3.0", "x"), ""), (os.path.join(HOME, ".local", "share", "fonts", "f"), ""),
          (os.path.join(W, "run", "wayland-0"), "")],
   dirs=[os.path.join(HOME, "Docs"), os.path.join(HOME, ".config", "demo"), os.path.join(HOME, ".cache", "demowin"), os.path.join(HOME, ".config", "Demo-App"),
         os.path.join(HOME, ".var", "app", "demo")])
la("the sandbox, the whole home, others seen", [{"args": ["demo"]}], files=[desktop("demo", "Exec=demo\n"), (os.path.join(HOME, ".config", "labyrinth", "apps", "demo.json"), {"sandbox": 1})])
la("the sandbox asked for, bwrap missing", [{"args": ["demo"]}], files=[desktop("demo", "Exec=demo\n"), (os.path.join(HOME, ".config", "labyrinth", "apps", "demo.json"), {"sandbox": True})],
   tools=("systemd-run", "demo"))
la("the sandbox and no systemd-run: the app alone", [{"args": ["demo"]}],
   files=[desktop("demo", "Exec=demo\n"), (os.path.join(HOME, ".config", "labyrinth", "apps", "demo.json"), {"sandbox": True, "folders": "ab"})], tools=("demo", "bwrap"))
la("policies: folders as text, as an object, odd values", [{"args": ["demo"]}, {"args": ["demo2"]}, {"args": ["demo3"]}],
   files=[desktop("demo", "Exec=demo\n"), desktop("demo2", "Exec=demo\n"), desktop("demo3", "Exec=demo\n"),
          (os.path.join(HOME, ".config", "labyrinth", "apps", "demo.json"), {"sandbox": True, "files": "folders", "folders": "/x"}),
          (os.path.join(HOME, ".config", "labyrinth", "apps", "demo2.json"), {"sandbox": True, "files": "folders", "folders": {"/etc": 1}}),
          (os.path.join(HOME, ".config", "labyrinth", "apps", "demo3.json"), {"sandbox": True, "files": "folders", "folders": None})])
la("a policy that is not an object or not JSON", [{"args": ["demo"]}, {"args": ["demo2"]}],
   files=[desktop("demo", "Exec=demo\n"), desktop("demo2", "Exec=demo\n"), (os.path.join(HOME, ".config", "labyrinth", "apps", "demo.json"), "[1]"),
          (os.path.join(HOME, ".config", "labyrinth", "apps", "demo2.json"), "{")])
la("a network rule: the slice started, the rules loaded", [{"args": ["demo"]}, {"args": ["demo2"]}, {"args": ["demo3"]}],
   files=[desktop("demo", "Exec=demo\n"), desktop("demo2", "Exec=demo\n"), desktop("demo3", "Exec=demo\n"),
          (STATE, {"rules": {UID: {"demo": "block", "demo2": "ask", "demo3": "wifi"}}})])
la("network rules that are odd", [{"args": ["demo"]}, {"args": ["demo"], "env": {"LABYRINTH_APPNET_STATE": os.path.join(W, "r2")}}],
   files=[desktop("demo", "Exec=demo\n"), (STATE, {"rules": [1]}), (os.path.join(W, "r2"), {"rules": {UID: ["demo"]}})])
for label, content in [("counted before", {"demo": {"launches": 4, "last": 1, "x": "keep"}, "other": {"launches": 1}}), ("not JSON", "{"),
                       ("launches as text", {"demo": {"launches": " 7"}}), ("launches as a float", {"demo": {"launches": 2.9}}),
                       ("an entry that is empty", {"demo": 0}), ("an entry that is a list", {"demo": [1]}), ("launches bad text", {"demo": {"launches": "x"}}),
                       ("the file a list", "[1]"), ("launches null", {"demo": {"launches": None}})]:
    la("app-usage.json: " + label, [{"args": ["demo"]}], files=[desktop("demo", "Exec=demo\n"), (os.path.join(HOME, ".local", "share", "labyrinth", "app-usage.json"), content)])
la("app-usage.json cannot be written", [{"args": ["demo"]}], files=[desktop("demo", "Exec=demo\n"), (os.path.join(HOME, ".local", "share", "labyrinth"), "a file")])
la("the program is missing", [{"args": ["demo"]}], files=[desktop("demo", "Exec=no-such-program\n")], tools=("demo",))
la("a working folder that is gone", [{"args": ["demo"]}], files=[desktop("demo", "Exec=demo\nPath=/nonexistent\n")])
la("a .desktop file that is not UTF-8", [{"args": ["demo"]}], files=[(os.path.join(APPS, "demo.desktop"), b"[Desktop Entry]\nName=a\xffb\nExec=demo %c\n")])
