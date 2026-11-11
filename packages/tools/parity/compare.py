#!/usr/bin/env python3
"""compare: Labyrinth's Rust programs (packages/tools) and the Python scripts they replace, run as programs on the same made-up
worlds. Every tool they call is a stand-in in WORLD/bin that writes down how it was called (and what it read, when a case asks)
and answers as the case says; every file they read or write is under the scratch WORLD (made again from scratch, at the same
path, before each run) or, for moves across file systems, under packages/tools/target/parity-xdev. Nothing on the computer is
changed and nothing is installed.

For each case both run the same steps in the same world and must give the same exit codes, standard output, standard error, calls
to the stand-ins, and files afterwards (names, modes, contents), byte for byte, after these are made alike:

  - quarantine ids (12 random hex digits) become <ID1>, <ID2> ... in order of appearance; the local time "YYYY-MM-DD HH:MM:SS"
    becomes <TIME>, a date after "restored " <DATE>, and a backup's epoch seconds <EPOCH>;
  - where the script stopped with a traceback, the Rust program must stop with exit code 1 and its one line "unexpected error
    (the Python script stopped here with a traceback)": the two standard errors are not compared;
  - what a case lists in `differs` (a difference on purpose, said in the module's "Not 1:1").

    compare.py [--python DIR] [--bin DIR] [--only NAME[,NAME]] [--case TEXT] [-v] [--keep]

--python: the folder with the Python scripts (default: ../python, else the repository's profile/airootfs/usr/local/bin)
--bin: the folder with the Rust programs (default: ../target/release; build with cargo build --release)
"""
import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.dirname(HERE)
REPO = os.path.normpath(os.path.join(TOOLS, "..", ".."))
ARGS = None
SCRATCH = tempfile.mkdtemp(prefix="labyrinth-tools-parity-")
WORLD = os.path.join(SCRATCH, "world")
XDEV = os.path.join(TOOLS, "target", "parity-xdev")

STUB = r"""#!/bin/sh
# a stand-in: writes down how it was called, answers as the case says (OUT_x, ERR_x, RC_x; STDIN_x=1 records its input,
# STDIN_x=ARG only when its first argument is ARG)
name=$(basename "$0")
key=$(printf '%s' "$name" | tr -c 'A-Za-z0-9' '_')
line=$name
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$WORLD/calls"
eval "rd=\${STDIN_$key:-}"
if [ -n "$rd" ] && { [ "$rd" = 1 ] || [ "$rd" = "${1:-}" ]; }; then printf 'stdin: ' >> "$WORLD/calls"; cat >> "$WORLD/calls"; echo '<end>' >> "$WORLD/calls"; fi
eval "out=\${OUT_$key:-}"
[ -n "$out" ] && printf '%s' "$out"
eval "err=\${ERR_$key:-}"
[ -n "$err" ] && printf '%s' "$err" >&2
eval "rc=\${RC_$key:-0}"
exit "$rc"
"""


def write(path, data, mode=None):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data.encode() if isinstance(data, str) else data)
    if mode is not None:
        os.chmod(path, mode)


def stubs(names):
    for n in names:
        write(os.path.join(WORLD, "bin", n), STUB, 0o755)


def snapshot(root, skip=("bin", "calls")):
    out = {}
    if not os.path.isdir(root):
        return out
    for d, dirs, files in os.walk(root):
        dirs.sort()
        rel = os.path.relpath(d, root)
        if rel.split(os.sep)[0] in skip or "__pycache__" in rel:
            dirs[:] = []
            continue
        for n in sorted(files) + sorted(x for x in dirs if os.path.islink(os.path.join(d, x))):
            if rel == "." and n in skip:
                continue
            p = os.path.join(d, n)
            r = os.path.normpath(os.path.join(rel, n))
            if os.path.islink(p):
                out[r] = ["link", os.readlink(p)]
            else:
                try:
                    with open(p, "rb") as f:
                        body = f.read().decode("utf-8", "backslashreplace")
                except OSError as e:
                    body = "<cannot read: %s>" % e.strerror
                out[r] = [oct(os.stat(p).st_mode & 0o7777), body]
        for n in dirs:
            if not os.path.islink(os.path.join(d, n)):
                out[os.path.normpath(os.path.join(rel, n)) + "/"] = oct(os.stat(os.path.join(d, n)).st_mode & 0o7777)
    return out


def base_normalize(text):
    ids = {}

    def idsub(m):
        return ids.setdefault(m.group(0), "<ID%d>" % (len(ids) + 1))

    text = re.sub(r"(?<![0-9a-f])[0-9a-f]{12}(?![0-9a-f])", idsub, text)
    text = re.sub(r"\d{4}-\d\d-\d\d \d\d:\d\d:\d\d", "<TIME>", text)
    text = re.sub(r"restored \d{4}-\d\d-\d\d", "restored <DATE>", text)
    text = re.sub(r"labyrinth-build-[0-9a-z_]+", "labyrinth-build-<RANDOM>", text)
    text = re.sub(r"\.labyrinth-\d{9,}\.bak", ".labyrinth-<EPOCH>.bak", text)
    return text


class Case:
    """prog: the program the steps run (a step may name another with "prog"); steps: [{args, env, stdin, cwd, prog, wrap}];
    setup(paths): makes the world; env: for every step; differs: what is not compared ("calls", "tree", ("out", i), ("err", i));
    settle: seconds to wait after each step (for what a program left running); watch(paths): for a program that keeps
    running, what happens while it runs (it is stopped afterwards); sort_calls: the calls' order is not compared (threads);
    norm: more making alike, text -> text."""

    def __init__(self, prog, name, steps, setup=None, env=None, differs=(), settle=0.0, roots=(), watch=None, sort_calls=False, norm=None):
        self.prog, self.name, self.steps, self.setup = prog, name, steps, setup
        self.env, self.differs, self.settle, self.roots, self.watch = env or {}, differs, settle, roots, watch
        self.sort_calls, self.norm = sort_calls, norm


CASES = []


def case(*a, **k):
    CASES.append(Case(*a, **k))


def impl_paths(kind):
    """{program name: how to run it} for one implementation."""
    names = ["labyrinth-scan-downloads", "labyrinth-quarantine", "labyrinth-appnet", "labyrinth-appnet-ask", "labyrinth-appnet-ask-agent",
             "labyrinth-launch", "labyrinth-apps", "labyrinth-selfcheck", "ipconfig", "steam_playall.py", "labyrinth-first-components"]
    out = {}
    for n in names:
        if kind == "python":
            out[n] = os.path.join(ARGS.python, n)
        else:
            out[n] = os.path.join(ARGS.bin, n.replace(".py", ""))
    return out


def fill(value, paths):
    return (value.replace("{WORLD}", WORLD).replace("{XDEV}", XDEV).replace("{Q}", paths["labyrinth-quarantine"])
            .replace("{SCAN}", paths["labyrinth-scan-downloads"]).replace("{APPS}", paths["labyrinth-apps"]))


def wipe(d):
    """Remove a world, also the folders a case made unwritable."""
    if not os.path.lexists(d):
        return
    for root, dirs, _ in os.walk(d):
        for n in dirs:
            p = os.path.join(root, n)
            if not os.path.islink(p):
                os.chmod(p, 0o700)
    os.chmod(d, 0o700)
    shutil.rmtree(d)


def run_case(c, kind):
    for d in (WORLD, XDEV):
        wipe(d)
    os.makedirs(os.path.join(WORLD, "home"))
    os.makedirs(XDEV)
    paths = impl_paths(kind)
    if c.setup:
        c.setup(paths)
    base = {"PATH": os.path.join(WORLD, "bin") + ":/usr/bin:/bin", "HOME": os.path.join(WORLD, "home"), "WORLD": WORLD,
            "LANG": "en_US.UTF-8", "TZ": "America/Denver", "USER": "tester"}
    results = []
    for step in c.steps:
        prog = step.get("prog", c.prog)
        env = dict(base)
        for k, v in list(c.env.items()) + list(step.get("env", {}).items()):
            if v is None:
                env.pop(k, None)
            else:
                env[k] = fill(v, paths)
        exe = [paths[prog]] if os.access(paths[prog], os.X_OK) else ["python3", paths[prog]]  # a script git keeps without its x bit
        argv = step.get("wrap", []) + exe + [fill(a, paths) for a in step.get("args", [])]
        cwd = fill(step.get("cwd", WORLD), paths)
        if c.watch:
            p = subprocess.Popen(argv, env=env, cwd=cwd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            c.watch(paths)
            os.killpg(p.pid, signal.SIGTERM)
            out, err = p.communicate()
            rc = p.returncode
        else:
            try:
                p = subprocess.run(argv, env=env, cwd=cwd, capture_output=True, input=step.get("stdin", "").encode(), timeout=120)
            except subprocess.TimeoutExpired:
                results.append({"rc": "timeout"})
                continue
            out, err, rc = p.stdout, p.stderr, p.returncode
        if c.settle:
            time.sleep(c.settle)
        results.append({"rc": rc, "out": out.decode("utf-8", "backslashreplace"), "err": err.decode("utf-8", "backslashreplace")})
    calls = open(os.path.join(WORLD, "calls"), encoding="utf-8", errors="backslashreplace").read() if os.path.exists(os.path.join(WORLD, "calls")) else ""
    tree = {"world": snapshot(WORLD), "xdev": snapshot(XDEV)}
    return results, calls, tree


def same(c, py, rs):
    """None when the two runs agree, else what differs."""
    (pr, pc, pt), (rr, rc_, rt) = py, rs
    extra = c.norm or (lambda t: t)

    def normalize(t):
        return extra(base_normalize(t))

    if c.sort_calls:
        pc, rc_ = "".join(sorted(pc.splitlines(True))), "".join(sorted(rc_.splitlines(True)))
    problems = []
    for i, (a, b) in enumerate(zip(pr, rr)):
        if a.get("rc") != b.get("rc"):
            problems.append("step %d: exit %s vs %s" % (i, a.get("rc"), b.get("rc")))
        crash = a.get("rc") == 1 and "Traceback (most recent call last)" in a.get("err", "")
        if crash:
            if "unexpected error (the Python script stopped here with a traceback)" not in b.get("err", ""):
                problems.append("step %d: the script crashed, the Rust program did not say so: %r" % (i, b.get("err")))
        elif ("err", i) not in c.differs and normalize(a.get("err", "")) != normalize(b.get("err", "")):
            problems.append("step %d: stderr\n  py: %r\n  rs: %r" % (i, a.get("err"), b.get("err")))
        if ("out", i) not in c.differs and normalize(a.get("out", "")) != normalize(b.get("out", "")):
            problems.append("step %d: stdout\n  py: %r\n  rs: %r" % (i, a.get("out"), b.get("out")))
    if "calls" not in c.differs and normalize(pc) != normalize(rc_):
        problems.append("calls\n  py: %r\n  rs: %r" % (pc, rc_))
    pj, rj = normalize(json.dumps(pt, sort_keys=True, indent=1)), normalize(json.dumps(rt, sort_keys=True, indent=1))
    if "tree" not in c.differs and pj != rj:
        import difflib
        problems.append("files afterwards:\n" + "\n".join(difflib.unified_diff(pj.splitlines(), rj.splitlines(), "python", "rust", lineterm="", n=1)))
    return "\n".join(problems) or None


# ---- the cases

sys.path.insert(0, HERE)
import cases_scan  # noqa: E402,F401  (each cases_*.py adds its cases with case())
import cases_appnet  # noqa: E402,F401
import cases_apps  # noqa: E402,F401


def main():
    global ARGS
    ap = argparse.ArgumentParser()
    ap.add_argument("--python")
    ap.add_argument("--bin", default=os.path.join(TOOLS, "target", "release"))
    ap.add_argument("--only")
    ap.add_argument("-v", action="store_true")
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--case", help="only the cases whose name has this text")
    ARGS = ap.parse_args()
    if not ARGS.python:
        own = os.path.join(TOOLS, "python")
        ARGS.python = own if os.path.isdir(own) else os.path.join(REPO, "profile", "airootfs", "usr", "local", "bin")
    only = set(ARGS.only.split(",")) if ARGS.only else None
    counts, failed = {}, 0
    try:
        for c in CASES:
            if (only and c.prog not in only) or (ARGS.case and ARGS.case not in c.name):
                continue
            py = run_case(c, "python")
            rs = run_case(c, "rust")
            bad = same(c, py, rs)
            n = counts.setdefault(c.prog, [0, 0])
            n[0] += 1
            if bad:
                failed += 1
                print("DIFFERENT  %s: %s\n%s" % (c.prog, c.name, bad))
            else:
                n[1] += 1
                if ARGS.v:
                    print("same       %s: %s" % (c.prog, c.name))
    finally:
        if not ARGS.keep:
            wipe(XDEV)
            wipe(SCRATCH)
    for prog, (n, ok) in sorted(counts.items()):
        print("%-28s %d/%d cases the same" % (prog, ok, n))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
