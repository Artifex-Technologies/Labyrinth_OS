"""The cases of labyrinth-scan-downloads and labyrinth-quarantine (loaded by compare.py). Malware is never used: the scanner is a
stand-in that reports what the case says, and the one run of the real scanner (heimdalld with ClamAV) scans ClamAV's EICAR test
string."""
import os
import time

import __main__ as P

W = P.WORLD
HOME = os.path.join(W, "home")
DL = os.path.join(HOME, "Downloads")
CONF = os.path.join(HOME, ".config", "labyrinth")
STATE = os.path.join(HOME, ".local", "state", "labyrinth")
QDATA = os.path.join(HOME, ".local", "share", "labyrinth", "quarantine")
TOOLS = ["heimdalld", "notify-send", "systemctl"]
EICAR = "X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*"

SCAN_ENV = {"HEIMDALLD": os.path.join(W, "bin", "heimdalld"), "LABYRINTH_QUARANTINE": "{Q}", "LABYRINTH_RUN_MEDIA": os.path.join(W, "media"),
            "LABYRINTH_SCAN_SYNC": "1", "RC_heimdalld": "1"}


def report(*found, summary="scanned 3 files in 1s: 1 infected, 0 could not be scanned"):
    return "".join("INFECTED %s: %s\n" % f for f in found) + summary + "\n"


def setup_files(files=(), dirs=(), links=(), modes=()):
    def go(paths):
        P.stubs(TOOLS)
        for d in dirs:
            os.makedirs(d, exist_ok=True)
        for p, body, *mode in files:
            P.write(p, body, mode[0] if mode else 0o644)
        for target, link in links:
            os.makedirs(os.path.dirname(link), exist_ok=True)
            os.symlink(target, link)
        for p, m in modes:
            os.chmod(p, m)
    return go


def scan_case(name, found_files, rep, env=None, steps=None, dirs=(DL,), extra_files=(), links=(), **k):
    e = dict(SCAN_ENV)
    e["OUT_heimdalld"] = rep
    e.update(env or {})
    P.case("labyrinth-scan-downloads", name, steps or [{}], setup=setup_files(list(found_files) + list(extra_files), dirs, links), env=e, **k)


bad = os.path.join(DL, "bad.exe")
scan_case("a find outside the watched places: a warning, once", [], report(("/home/u/Downloads/bad.exe", "Test.Virus")), steps=[{}, {}])
scan_case("a certain find in Downloads: quarantined, Restore/Delete asked, nothing answered", [(bad, "evil\n", 0o755)], report((bad, "Win.Trojan.Agent-123")),
          steps=[{}, {"prog": "labyrinth-quarantine", "args": ["list", "--json"]}])
scan_case("the notification's Restore: back, remembered, not caught again", [(bad, "evil\n", 0o755)], report((bad, "Win.Trojan.Agent-123")),
          steps=[{"env": {"OUT_notify_send": "restore\n"}}, {}, {"prog": "labyrinth-quarantine", "args": ["list"]}])
scan_case("the notification's Delete", [(bad, "evil\n", 0o755)], report((bad, "Win.Trojan.Agent-123")), env={"OUT_notify_send": "delete\n"},
          steps=[{}, {"prog": "labyrinth-quarantine", "args": ["list", "--json"]}])
for sig in ["PUA.Win.Packer.Upx", "Heuristics.Encrypted.PDF", "Win.Test.EICAR_HDB-1", "Html.Phishing.Bank-9", "Doc.Exceeds.Limit", "WIN.TROJAN.HEURISTIC"]:
    scan_case("a guess is only a warning: " + sig, [(bad, "x", 0o755)], report((bad, sig)))
scan_case("action = notify: never moved", [(bad, "x"), (os.path.join(CONF, "scan.conf"), "# mine\naction = notify\n")], report((bad, "Win.Trojan.A")))
scan_case("scan.conf: last action wins, odd lines", [(bad, "x"), (os.path.join(CONF, "scan.conf"), "action=notify\nnonsense\n  action =quarantine  \n=\n")],
          report((bad, "Win.Trojan.A")))
scan_case("scan.conf: an unknown action is notify-like", [(bad, "x"), (os.path.join(CONF, "scan.conf"), "action = maybe\n")], report((bad, "Win.Trojan.A")))
mail = os.path.join(HOME, ".cache", "thunderbird", "p", "attach.scr")
scan_case("a mail attachment cache is a watched place", [(mail, "x")], report((mail, "Win.Trojan.Agent-9")), dirs=(DL, os.path.join(W, "media")))
media = os.path.join(W, "media", "USB", "setup.exe")
scan_case("a drive is a watched place", [(media, "x")], report((media, "Win.Trojan.Agent-9")))
scan_case("an extra place from scan-folders, with ~", [(os.path.join(HOME, "Extra", "x.exe"), "x"), (os.path.join(CONF, "scan-folders"), "~/Extra\n#~/Nope\n\n")],
          report((os.path.join(HOME, "Extra", "x.exe"), "Win.Trojan.A")))
real_dl = os.path.join(W, "elsewhere", "dl")
scan_case("Downloads is a link: its real folder is the place", [(os.path.join(real_dl, "x.exe"), "x")],
          report((os.path.join(DL, "x.exe"), "Win.Trojan.A")), dirs=(real_dl,), links=[(real_dl, DL)])
scan_case("a link in Downloads is never moved", [(os.path.join(W, "t.bin"), "x")], report((os.path.join(DL, "l.exe"), "Win.Trojan.A")),
          links=[(os.path.join(W, "t.bin"), os.path.join(DL, "l.exe"))])
scan_case("a file already allowed (restored before) is skipped", [(bad, "evil\n"),
          (os.path.join(CONF, "scan-allow"), "a  # x\n%s  # restored 2026-01-01: bad.exe\n" % "4d7c1d6b0b3d7e11ecbcf2a8a2a0e3a5e4af0b0ee4bb7b9cf06e2d4d2c3d7a1f")],
          report((bad, "Win.Trojan.A")), steps=[{"prog": "labyrinth-quarantine", "args": ["allowed", "x"]}, {}])
scan_case("a file already allowed (its real hash)", [(bad, "evil\n")], report((bad, "Win.Trojan.A")),
          steps=[{"prog": "labyrinth-quarantine", "args": ["add", bad, "--signature", "S"]},
                 {"prog": "labyrinth-quarantine", "args": ["list"]}, {}])
scan_case("the quarantine cannot take it: logged, warned", [(bad, "evil\n"), (QDATA, "a file, not a folder")], report((bad, "Win.Trojan.A")))
scan_case("several finds, one notified before", [(bad, "1"), (os.path.join(DL, "b.exe"), "2"), (os.path.join(STATE, "scan-notified.json"), '["/x", "%s"]' % bad)],
          report((bad, "PUA.A"), (os.path.join(DL, "b.exe"), "PUA.B"), ("/z/c.exe", "PUA.C")))
scan_case("the same find twice in one report", [(bad, "1")], report((bad, "PUA.A"), (bad, "PUA.A")))
for label, content in [("an object", '{"/a": 1}'), ("text", '"abc"'), ("not JSON", "{"), ("a number", "5"), ("null", "null"),
                       ("a list with a number", '[1, "/a"]'), ("a list with a list", '[["a"]]'), ("not UTF-8", b"[\xff]")]:
    scan_case("scan-notified.json is " + label, [(bad, "1"), (os.path.join(STATE, "scan-notified.json"), content)], report((bad, "PUA.A")))
    scan_case("scan-notified.json is %s, nothing found" % label, [(os.path.join(STATE, "scan-notified.json"), content)], "scanned 1 files in 1s: 0 infected\n",
              env={"RC_heimdalld": "0"})
scan_case("the scanner cannot run (exit 2): its first line logged", [], "", env={"RC_heimdalld": "2", "ERR_heimdalld": "\n  ClamAV is not installed\nsecond\n"})
scan_case("the scanner cannot run, says nothing", [], "", env={"RC_heimdalld": "2"})
scan_case("the scanner is missing", [], "", env={"HEIMDALLD": "no-such-heimdalld"})
scan_case("the scanner is not executable", [(os.path.join(W, "heim.txt"), "x")], "", env={"HEIMDALLD": os.path.join(W, "heim.txt")})
scan_case("nothing found, 0 files: no summary logged", [], "scanned 0 files in 0s\n", env={"RC_heimdalld": "0"})
scan_case("nothing found: the last summary logged", [], "scanned 1 files\nnoise\nscanned 2 files in 1s: 0 infected\n", env={"RC_heimdalld": "0"})
scan_case("an odd report: no colon, empty path, CRLF", [], "INFECTED nothing\r\nINFECTED : Sig\r\nINFECTED /a: b: c\r\nscanned 1 files\r\n")
scan_case("no watched place exists: no scan at all", [], report(("/x", "Y")), dirs=())
scan_case("scan-exclude and scan-folders: comments, blanks, ~, an indented #", [
    (os.path.join(CONF, "scan-exclude"), "# comment\n*.iso\n\n   \n~/Downloads/big\n  # not a comment\r\nlast-no-newline"),
    (os.path.join(CONF, "scan-folders"), "%s\n~\n~/missing\n" % os.path.join(W, "extra"))], report(), dirs=(DL, os.path.join(W, "extra")), env={"RC_heimdalld": "0"})
scan_case("scan-exclude is not UTF-8", [(os.path.join(CONF, "scan-exclude"), b"a\xffb\n")], report())
scan_case("the weekly full scan", [(os.path.join(CONF, "scan-exclude"), "*.mkv\n")], report(("/elsewhere/x", "Win.Trojan.A")), steps=[{"args": ["--full"]}])
scan_case("XDG folders and an empty XDG_CONFIG_HOME", [(bad, "x")], report((bad, "Win.Trojan.A")),
          env={"XDG_STATE_HOME": os.path.join(W, "state"), "XDG_CONFIG_HOME": "", "XDG_DATA_HOME": os.path.join(W, "data")},
          steps=[{}, {"prog": "labyrinth-quarantine", "args": ["list"]}])
scan_case("RUN_MEDIA from USER, then from HOME's name", [], report(), env={"LABYRINTH_RUN_MEDIA": None, "RC_heimdalld": "0"}, steps=[{}, {"env": {"USER": None}}])
scan_case("--off and --on", [], "", steps=[{"args": ["--off"]}, {"args": ["--on"], "env": {"RC_systemctl": "3", "OUT_systemctl": "said\n"}}])
scan_case("--prompt with its three values (the child's own call): a scan, as the script did", [(bad, "x")], report((bad, "Win.Trojan.A")),
          env={"LABYRINTH_SCAN_PROMPT": "child", "LABYRINTH_SCAN_SYNC": None}, steps=[{"args": ["--prompt", "abcdefabcdef", "bad.exe", "Sig"]}])
scan_case("--prompt with a fourth value: the question, answered Delete", [(os.path.join(QDATA, "abcdefabcdef", "payload"), "x"),
          (os.path.join(QDATA, "abcdefabcdef", "info.json"), '{"id": "abcdefabcdef", "when": "2026-01-01 00:00:00", "original": "%s", "sha256": "s"}' % bad)],
          "", env={"LABYRINTH_SCAN_PROMPT": "child", "OUT_notify_send": "delete\n"}, steps=[{"args": ["--prompt", "abcdefabcdef", "bad.exe", "Sig", "x"]}])
scan_case("--prompt last: the script's IndexError", [], "", steps=[{"args": ["a", "b", "c", "d", "--prompt"]}])
scan_case("not waiting: the question in a child of its own (which scans again)", [(bad, "evil\n")], report((bad, "Win.Trojan.Agent-123")),
          env={"LABYRINTH_SCAN_SYNC": None}, settle=1.5)
scan_case("not waiting, a warning: notify-send left running", [(bad, "x")], report((bad, "PUA.A")), env={"LABYRINTH_SCAN_SYNC": None}, settle=0.5)
# a PATH with python3 alone (for the script's #!/usr/bin/env python3): notify-send is really missing, none is ever started
scan_case("notify-send is missing (not waiting)", [(bad, "x")], report((bad, "PUA.A")), env={"LABYRINTH_SCAN_SYNC": None, "PATH": os.path.join(W, "pybin")},
          links=[("/usr/bin/python3", os.path.join(W, "pybin", "python3"))])
scan_case("a quarantine program that is missing", [(bad, "x")], report((bad, "Win.Trojan.A")), env={"LABYRINTH_QUARANTINE": "no-such-q"})
scan_case("moved across file systems (the quarantine on another one)", [(bad, "evil\n", 0o750)], report((bad, "Win.Trojan.A")),
          env={"XDG_DATA_HOME": "{XDEV}"}, steps=[{}, {"prog": "labyrinth-quarantine", "args": ["list"], "env": {"XDG_DATA_HOME": "{XDEV}"}}])


def real_scanner_setup(paths):
    setup_files([(os.path.join(DL, "eicar.com"), EICAR), (os.path.join(DL, "clean.txt"), "hello\n")], (DL,))(paths)
    os.remove(os.path.join(W, "bin", "heimdalld"))


if os.path.exists("/usr/bin/heimdalld") and os.path.exists("/usr/bin/clamscan") and not os.environ.get("PARITY_NO_CLAMAV"):
    # ClamAV names it Eicar-Test-Signature, which counts as certain: it is quarantined. The tree is not compared: heimdalld's
    # state file holds times.
    P.case("labyrinth-scan-downloads", "the real scanner (heimdalld, ClamAV) on the EICAR test string: quarantined",
           [{}, {}], setup=real_scanner_setup, env={"HEIMDALLD": "/usr/bin/heimdalld", "LABYRINTH_QUARANTINE": "{Q}",
                                                    "LABYRINTH_RUN_MEDIA": os.path.join(W, "media"), "LABYRINTH_SCAN_SYNC": "1"},
           differs={"tree"})


def watch_actions(paths):
    time.sleep(1.0)
    os.makedirs(os.path.join(DL, "sub", "deeper"))
    time.sleep(0.5)
    P.write(os.path.join(DL, "sub", "deeper", "a.exe"), "x")
    time.sleep(7.5)
    P.write(os.path.join(HOME, "Desktop", "b.exe"), "y")
    os.rename(os.path.join(HOME, "Desktop", "b.exe"), os.path.join(HOME, "Desktop", "c.exe"))
    time.sleep(7.5)


P.case("labyrinth-scan-downloads", "the watcher: a new file deep in Downloads, then one on the Desktop",
       [{"args": ["--watch"]}], setup=setup_files([], (DL, os.path.join(HOME, "Desktop"))),
       env=dict(SCAN_ENV, OUT_heimdalld="scanned 1 files in 0s: 0 infected\n", RC_heimdalld="0"), watch=watch_actions)

# ---- labyrinth-quarantine on its own

Q = "labyrinth-quarantine"


def qcase(name, steps, files=(), dirs=(), links=(), env=None, modes=(), **k):
    P.case(Q, name, steps, setup=setup_files(files, dirs, links, modes), env=env, **k)


def info(qid, when="2026-01-02 03:04:05", base=QDATA, **extra):
    import json
    d = {"id": qid, "original": os.path.join(DL, "f-" + qid), "signature": "Sig." + qid[:3], "sha256": "ab" * 32, "size": 3, "mode": 0o755,
         "when": when, "source": "scan"}
    d.update(extra)
    d = {k: v for k, v in d.items() if v is not None or k not in ("mode",)}
    return [(os.path.join(base, qid, "info.json"), json.dumps(d, indent=1)), (os.path.join(base, qid, "payload"), "abc", 0o600)]


A, B, C = "aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc"
for args in [[], ["-h"], ["--help"], ["nope"], ["add"], ["restore"], ["delete", "a", "b"], ["allowed"], ["list", "extra", "--json"]]:
    qcase("arguments: %r" % args, [{"args": args}])
qcase("add, then list (text and JSON)", [{"args": ["add", bad, "--signature", "Win.Trojan.Test"]}, {"args": ["list"]}, {"args": ["list", "--json"]}],
      files=[(bad, "evil\n", 0o4755)], dirs=(DL,))
qcase("add without --signature, a relative path", [{"args": ["add", "Downloads/bad.exe"], "cwd": HOME}, {"args": ["list"]}], files=[(bad, "evil\n")])
qcase("add with --signature last: the script's IndexError", [{"args": ["add", bad, "--signature"]}], files=[(bad, "evil\n")])
qcase("add a link, a folder, a missing file, an unreadable file", [{"args": ["add", os.path.join(DL, "l"), "--signature", "X"]},
      {"args": ["add", DL, "--signature", "X"]}, {"args": ["add", os.path.join(DL, "nope"), "--signature", "X"]},
      {"args": ["add", os.path.join(DL, "locked"), "--signature", "X"]}],
      files=[(os.path.join(W, "t"), "x"), (os.path.join(DL, "locked"), "x", 0o000)], links=[(os.path.join(W, "t"), os.path.join(DL, "l"))])
qcase("add across file systems keeps the times; restore across them too", [{"args": ["add", bad, "--signature", "S"]}, {"args": ["list"]}],
      files=[(bad, "evil\n", 0o640)], env={"XDG_DATA_HOME": "{XDEV}"})
qcase("add when the quarantine folder cannot be made", [{"args": ["add", bad, "--signature", "S"]}], files=[(bad, "x"), (os.path.join(HOME, ".local", "share", "labyrinth"), "file")])
qcase("list: newest first, ties in name order, others skipped", [{"args": ["list"]}, {"args": ["list", "--json"]}],
      files=info(B, "2026-01-01 00:00:00") + info(A, "2026-01-01 00:00:00") + info(C, "2026-02-01 00:00:00", signature="Sig.café \U0001F600")
      + [(os.path.join(QDATA, "dddddddddddd", "info.json"), '{"id": "other"}'), (os.path.join(QDATA, "eeeeeeeeeeee", "info.json"), "not json"),
         (os.path.join(QDATA, "notanid", "info.json"), '{"id": "notanid", "when": "x"}'), (os.path.join(QDATA, "ffffffffffff", "info.json"), b"\xff")])
qcase("list: one item without \"when\" is fine, two are not", [{"args": ["list"]}], files=[(os.path.join(QDATA, A, "info.json"), '{"id": "%s"}' % A)])
qcase("list: no \"when\" with two items", [{"args": ["list"]}], files=info(A) + [(os.path.join(QDATA, B, "info.json"), '{"id": "%s"}' % B)])
qcase("list: \"when\" as numbers, as a number and text", [{"args": ["list", "--json"]}], files=info(A, 5) + info(B, 7.5))
qcase("list: \"when\" as a number and text", [{"args": ["list"]}], files=info(A, 5) + info(B, "x"))
qcase("list: an info.json that is a list", [{"args": ["list"]}], files=info(A) + [(os.path.join(QDATA, B, "info.json"), "[1]")])
qcase("list: text listing with a missing signature", [{"args": ["list"]}], files=info(A) + [(os.path.join(QDATA, B, "info.json"), '{"id": "%s", "when": "z"}' % B)])
qcase("list: values that are not text", [{"args": ["list"]}], files=info(A, signature=None, original=["a", 1.5]))
qcase("restore: back where it was, without the execute bit, remembered", [{"args": ["restore", A]}, {"args": ["list"]}, {"args": ["allowed", "ab" * 32]}],
      files=info(A) + [(os.path.join(CONF, "scan-allow"), "old\n")], dirs=(DL,))
qcase("restore beside what is there: .restored, .restored2, .restored3", [{"args": ["restore", A]}],
      files=info(A) + [(os.path.join(DL, "f-" + A), "1"), (os.path.join(DL, "f-%s.restored" % A), "2"), (os.path.join(DL, "f-%s.restored2" % A), "3")])
qcase("restore into a folder that is gone", [{"args": ["restore", A]}], files=info(A, original=os.path.join(W, "gone", "deep", "f")))
qcase("restore without a payload", [{"args": ["restore", A]}], files=[info(A)[0]])
qcase("restore: mode missing, mode as text, mode true", [{"args": ["restore", A]}, {"args": ["restore", B]}, {"args": ["restore", C]}],
      files=info(A, mode=None) + info(B, mode="755") + info(C, mode=True), dirs=(DL,))
qcase("restore: original is not text", [{"args": ["restore", A]}], files=info(A, original=5))
qcase("restore across file systems", [{"args": ["restore", A]}, {"args": ["list"]}],
      files=info(A, base=os.path.join(P.XDEV, "labyrinth", "quarantine")), dirs=(DL,), env={"XDG_DATA_HOME": "{XDEV}"})
for bad_id in ["../../etc", "zzzz", "", "123456789012", "aaaaaaaaaaaa\n", "AAAAAAAAAAAA"]:
    qcase("restore and delete a bad or unknown id: %r" % bad_id, [{"args": ["restore", bad_id]}, {"args": ["delete", bad_id]}], files=info(A))
qcase("delete", [{"args": ["delete", A]}, {"args": ["list", "--json"]}], files=info(A) + info(B))
qcase("delete what cannot be deleted", [{"args": ["delete", A]}, {"args": ["list"]}], files=info(A) + [(os.path.join(QDATA, A, "sub", "x"), "y")],
      modes=[(os.path.join(QDATA, A, "sub"), 0o500)])
qcase("allowed: hashes, comments, spaces, CRLF", [{"args": ["allowed", "abc"]}, {"args": ["allowed", "def"]}, {"args": ["allowed", "ghi"]}, {"args": ["allowed", ""]}],
      files=[(os.path.join(CONF, "scan-allow"), "abc  # restored\r\n  def\t\n#ghi\n\n")])
qcase("allowed: no file; a folder; not UTF-8", [{"args": ["allowed", "x"]}, {"args": ["allowed", "x"], "env": {"XDG_CONFIG_HOME": os.path.join(W, "c2")}},
      {"args": ["allowed", "x"], "env": {"XDG_CONFIG_HOME": os.path.join(W, "c3")}}],
      files=[(os.path.join(W, "c3", "labyrinth", "scan-allow"), b"x\n\xff\n")], dirs=(os.path.join(W, "c2", "labyrinth", "scan-allow"),))
