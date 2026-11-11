#!/usr/bin/env python3
"""bench-scripts: the shell scripts of profile/airootfs/usr/local/{bin,sbin} against their Rust ports (packages/tools), timed
on the parity cases' own worlds (parity/cases_root.py: a user, mount and network namespace with a root folder of its own and
stand-ins for every outside program). Nothing on this computer is changed and nothing reaches the network.

    cargo build --release && python3 -I parity/bench-scripts.py [--runs 30] [--commit REV] [--only NAME[,NAME]] [--json FILE]

- The old side is each script as git has it at --commit (default HEAD), written to a scratch folder; the new side is
  target/release.
- Each case: its world is made (again before every run when the program changes it), one unmeasured run of each (their exit
  codes and output compared), then RUNS of each, alternating which goes first, one process at a time, at nice 19.
- A small C launcher (built with cc), run inside the world in place of the program, starts it and reports its wall time, user +
  system CPU and peak RSS from wait4: the namespace's own making is not measured. CPU and peak RSS include the stand-ins a
  program starts (the same calls on both sides; the stand-ins are a small C program answering from files, about 0.3 ms, and
  the parity cases' shell stand-in where a case needs one to act: a mount, a snapshot).
- Medians, p10-p90 of the wall time in brackets.
"""
import argparse
import importlib.util
import json
import os
import re
import statistics
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.dirname(HERE)
REPO = os.path.normpath(os.path.join(TOOLS, "..", ".."))

LAUNCH_C = r"""
#define _GNU_SOURCE
#include <stdio.h>
#include <time.h>
#include <unistd.h>
#include <sys/resource.h>
#include <sys/wait.h>
/* bench-launch RESULT PROGRAM [ARGS...]: runs PROGRAM, then writes to RESULT: wall_ns user_us sys_us maxrss_kb wait_status */
int main(int argc, char **argv) {
    struct timespec a, b;
    struct rusage ru;
    int st;
    if (argc < 3) return 125;
    clock_gettime(CLOCK_MONOTONIC, &a);
    pid_t pid = fork();
    if (pid < 0) return 126;
    if (pid == 0) { execv(argv[2], argv + 2); _exit(127); }
    if (wait4(pid, &st, 0, &ru) < 0) return 126;
    clock_gettime(CLOCK_MONOTONIC, &b);
    FILE *f = fopen(argv[1], "w");
    if (!f) return 126;
    fprintf(f, "%lld %lld %lld %ld %d\n", (long long)(b.tv_sec - a.tv_sec) * 1000000000LL + (b.tv_nsec - a.tv_nsec),
            (long long)ru.ru_utime.tv_sec * 1000000LL + ru.ru_utime.tv_usec,
            (long long)ru.ru_stime.tv_sec * 1000000LL + ru.ru_stime.tv_usec, ru.ru_maxrss, st);
    if (fclose(f) != 0) return 126;
    return WIFEXITED(st) ? WEXITSTATUS(st) : 128 + WTERMSIG(st);
}
"""

FAKE_C = r"""
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
/* bench-fake: the parity stand-in's answers (WORLD/fake/NAME_ARG1... .out .err .rc, '/' as '%', the longest run of arguments
   that has one) without a shell; where the answer is a .sh (an action), the shell stand-in WORLD/shfake/NAME does it. */
static int has(const char *k, const char *ext) { char p[8192]; snprintf(p, sizeof p, "%s%s", k, ext); return access(p, F_OK) == 0; }
static void cat(const char *k, const char *ext, FILE *to) {
    char p[8192], buf[65536]; size_t n; snprintf(p, sizeof p, "%s%s", k, ext);
    FILE *f = fopen(p, "r"); if (!f) return;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) fwrite(buf, 1, n, to);
    fclose(f);
}
int main(int argc, char **argv) {
    const char *w = getenv("WORLD");
    const char *name = strrchr(argv[0], '/');
    char key[8192], best[8192] = "";
    name = name ? name + 1 : argv[0];
    if (!w) return 125;
    int len = snprintf(key, sizeof key, "%s/fake/%s", w, name);
    for (int i = 0; i <= argc - 1 && len < (int)sizeof key; i++) {
        if (i > 0) {
            len += snprintf(key + len, sizeof key - len, "_%s", argv[i]);
            for (char *p = key + len - strlen(argv[i]); *p; p++) if (*p == '/') *p = '%';
        }
        if (has(key, ".out") || has(key, ".err") || has(key, ".rc") || has(key, ".sh") || has(key, ".stdin")) strcpy(best, key);
    }
    if (best[0] && (has(best, ".sh") || has(best, ".stdin"))) {
        char sh[8192]; snprintf(sh, sizeof sh, "%s/shfake/%s", w, name);
        execv(sh, argv);
        return 126;
    }
    if (!best[0]) return 0;
    cat(best, ".out", stdout); fflush(stdout);
    cat(best, ".err", stderr);
    if (has(best, ".rc")) { char p[8192]; snprintf(p, sizeof p, "%s.rc", best); FILE *f = fopen(p, "r"); int rc = 0; if (f) { if (fscanf(f, "%d", &rc) != 1) rc = 0; fclose(f); } return rc; }
    return 0;
}
"""

# (program, the case's name in the parity cases, which step, does the run change the world: then it is made again first)
CASES = [
    ("labyrinth-security-setting", "auto-quarantine status: written by hand", 0, False),
    ("labyrinth-sarina-setting", "forge keeps the other settings, no newline at the end", 0, False),
    ("labyrinth-sarina-setting", "voices: list, get, set", 0, False),
    ("labyrinth-agent-window", "status: open with no timer, or an odd list", 0, False),
    ("labyrinth-hardware-setup", "an ASUS laptop with AMD and NVIDIA", 1, False),
    ("labyrinth-gaming", "all there, Steam Play on", 1, False),
    ("labyrinth-rollback", "list: odd snapshots", 0, False),
    ("labyrinth-rollback", "dry run: the plan, nothing changed", 0, False),
    ("labyrinth-sarina-reflect", "the Inbox has files, settings from sarina.env", 0, False),
    ("labyrinth-sarina-warm", "the voice model from sarina.env", 0, False),
    ("labyrinth-rea", "everything: a JDK, Ghidra, /etc/java*, the quarantine, the network", 0, False),
    ("labyrinth-claude-code", "install, then status, then update", 1, False),
    ("labyrinth-install-cleanup", "a mock root, as a normal user (tests/test_install_cleanup.sh's way)", 0, True),
    ("labyrinth-install-bootfinish", "everything: btrfs, all units, ufw, theme", 0, True),
    ("labyrinth-agent-setup", "first set-up: the account made, rules, claude-sudo, Heimdall told", 0, True),
    ("labyrinth-ecosystem-setup", "everything installed, a base image", 0, True),
    ("labyrinth-grub-theme", "a connected 3440x1440 monitor, a disconnected one first", 0, True),
]


def load_compare(args):
    spec = importlib.util.spec_from_file_location("compare", os.path.join(HERE, "compare.py"))
    mod = importlib.util.module_from_spec(spec)
    sys.modules["__main__"] = mod  # the cases import __main__ as their harness
    sys.path.insert(0, HERE)
    spec.loader.exec_module(mod)
    mod.ARGS = argparse.Namespace(python=os.path.join(REPO, "profile", "airootfs", "usr", "local", "bin"), shell=args.old,
                                  bin=os.path.join(TOOLS, "target", "release"))
    return mod


def old_scripts(commit, dest):
    os.makedirs(dest, exist_ok=True)
    for n in (c[0] for c in CASES):
        where = "sbin" if n == "labyrinth-agent-window" else "bin"
        body = subprocess.run(["git", "-C", REPO, "show", "%s:profile/airootfs/usr/local/%s/%s" % (commit, where, n)], check=True,
                              capture_output=True).stdout
        with open(os.path.join(dest, n), "wb") as f:
            f.write(body)


def build(path, src):
    with open(path + ".c", "w") as f:
        f.write(src)
    subprocess.run(["cc", "-O2", "-o", path, path + ".c"], check=True)
    return path


def swap_fakes(P):
    """The world's shell stand-ins replaced by the C one (each kept in WORLD/shfake for the answers that act)."""
    b, sf = os.path.join(P.WORLD, "bin"), os.path.join(P.WORLD, "shfake")
    os.makedirs(sf, exist_ok=True)
    for n in os.listdir(b) if os.path.isdir(b) else []:
        p = os.path.join(b, n)
        if os.path.isfile(p) and not os.path.islink(p):
            os.replace(p, os.path.join(sf, n))
            os.symlink(FAKE, p)


def run_once(P, c, step, kind, paths):
    """One run of a case's step through the launcher; the measurements, exit code and output."""
    prog = step.get("prog", c.prog)
    env = {"PATH": os.path.join(P.WORLD, "bin") + ":/usr/bin:/bin", "HOME": os.path.join(P.WORLD, "home"), "WORLD": P.WORLD,
           "LANG": "en_US.UTF-8", "TZ": "America/Denver", "USER": "tester"}
    for k, v in list(c.env.items()) + list(step.get("env", {}).items()):
        if v is None:
            env.pop(k, None)
        else:
            env[k] = P.fill(v, paths)
    result = os.path.join(P.SCRATCH, "launch.result")
    if os.path.exists(result):
        os.remove(result)
    wrap = step.get("wrap", [])
    if wrap:  # inside the world: ns.sh starts the launcher, which starts the program
        env["INNER_LAUNCH"] = LAUNCH + " " + result
        argv = wrap + [paths[prog]] + step.get("args", [])
    else:
        exe = [paths[prog]] if os.access(paths[prog], os.X_OK) else P.interpreter(paths[prog]) + [paths[prog]]
        argv = [LAUNCH, result] + ([exe[0]] if len(exe) == 1 else ["/usr/bin/" + exe[0], exe[1]]) + step.get("args", [])
    p = subprocess.run(argv, env=env, input=step.get("stdin", "").encode(), capture_output=True, cwd=P.WORLD)
    with open(result) as f:
        wall, ut, st, rss, status = f.read().split()
    return {"wall": int(wall) / 1e9, "cpu": (int(ut) + int(st)) / 1e6, "rss": int(rss), "rc": os.waitstatus_to_exitcode(int(status)),
            "out": p.stdout, "err": p.stderr}


def stats(runs):
    wall = sorted(r["wall"] for r in runs)
    q = statistics.quantiles(wall, n=10, method="inclusive") if len(wall) > 1 else [wall[0]] * 9
    return {"runs": len(runs), "wall_ms": statistics.median(wall) * 1e3, "p10_ms": q[0] * 1e3, "p90_ms": q[8] * 1e3,
            "cpu_ms": statistics.median(r["cpu"] for r in runs) * 1e3, "rss_kb": statistics.median(r["rss"] for r in runs)}


def normal(b, P):
    b = re.sub(rb"\d{4}-\d\d-\d\d[T -]\d\d:?\d\d:?\d\d", b"<time>", b)
    b = re.sub(rb"/run/labyrinth-rollback\.[A-Za-z0-9]{6}", b"<tmp>", b)
    b = re.sub(rb"/tmp/tmp\.[A-Za-z0-9]{10}", b"<tmp>", b)
    return b.replace(P.SCRATCH.encode(), b"<scratch>")


def main():
    global LAUNCH, FAKE
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=30)
    ap.add_argument("--commit", default="HEAD")
    ap.add_argument("--only")
    ap.add_argument("--json")
    args = ap.parse_args()
    os.nice(19 - os.nice(0))
    import tempfile
    args.old = tempfile.mkdtemp(prefix="bench-scripts-old-")
    old_scripts(args.commit, args.old)
    P = load_compare(args)
    LAUNCH = build(os.path.join(P.SCRATCH, "bench-launch"), LAUNCH_C)
    FAKE = build(os.path.join(P.SCRATCH, "bench-fake"), FAKE_C)
    # ns.sh runs the program through the launcher when INNER_LAUNCH is set
    import cases_root as C
    C.NS = C.NS.replace('exec /usr/bin/chroot "$R" /usr/bin/env -C "$cwd" "$W/ov/prog" "$@"',
                        'exec /usr/bin/chroot "$R" /usr/bin/env -C "$cwd" $INNER_LAUNCH "$W/ov/prog" "$@"')
    assert "$INNER_LAUNCH" in C.NS
    only = set(args.only.split(",")) if args.only else None
    out = []
    try:
        for prog, name, si, changes in CASES:
            if only and prog not in only:
                continue
            c = next(x for x in P.CASES if x.prog == prog and x.name == name)
            step = c.steps[si]
            paths = {"old": P.impl_paths("python"), "new": P.impl_paths("rust")}

            def fresh():
                P.wipe(P.WORLD)
                P.wipe(P.XDEV)
                os.makedirs(os.path.join(P.WORLD, "home"))
                os.makedirs(P.XDEV)
                if c.setup:
                    c.setup(paths["old"])
                swap_fakes(P)

            def one(side):
                if changes:
                    if c.teardown:
                        c.teardown()
                    fresh()
                return run_once(P, c, step, side, paths[side])

            fresh()
            first = {s: one(s) for s in ("old", "new")}
            same = first["old"]["rc"] == first["new"]["rc"] and normal(first["old"]["out"], P) == normal(first["new"]["out"], P)
            res = {"old": [], "new": []}
            for i in range(args.runs):
                for s in (("old", "new") if i % 2 == 0 else ("new", "old")):
                    res[s].append(one(s))
            if c.teardown:
                c.teardown()
            row = {"program": prog, "case": name, "step": si, "same_output": same, "old": stats(res["old"]), "new": stats(res["new"])}
            out.append(row)
            o, n = row["old"], row["new"]
            print("%-30s %-60.60s %7.2f ms (%5.2f-%6.2f) -> %6.2f ms (%5.2f-%6.2f)  cpu %6.2f -> %6.2f ms  rss %6.1f -> %5.1f MB  %s" % (
                prog, name, o["wall_ms"], o["p10_ms"], o["p90_ms"], n["wall_ms"], n["p10_ms"], n["p90_ms"], o["cpu_ms"], n["cpu_ms"],
                o["rss_kb"] / 1024, n["rss_kb"] / 1024, "same output" if same else "OUTPUT DIFFERS"), flush=True)
    finally:
        P.wipe(P.XDEV)
        P.wipe(P.SCRATCH)
        import shutil
        shutil.rmtree(args.old, ignore_errors=True)
    if args.json:
        with open(args.json, "w") as f:
            json.dump(out, f, indent=1)


if __name__ == "__main__":
    main()
