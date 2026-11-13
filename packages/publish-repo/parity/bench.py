#!/usr/bin/python3 -I
"""bench.py: scripts/publish-repo.py against labyrinth-publish-repo, timed on a made-up world (compare.py's: a user, mount,
PID and network namespace, a tmpfs on /mnt, a throw-away key; the real keyring and gh's settings covered).

    cargo build --release && python3 -I parity/bench.py [--runs 20] [--old FILE] [--new FILE]

The world is made by one real publish (the new program, compare.py's stand-ins), then the stand-ins are swapped for small
shell ones (git, gh, makepkg, build-local-packages.sh answer from files in about a millisecond) so they weigh little next to
the program; gpg, bsdtar, gzip, vercmp and repo-add stay the real ones. Cases:

- help: --help (starting up and stopping);
- unchanged: the daily run when nothing moved on GitHub (the key checked, five sources fetched, nothing more);
- force: --force with 30 packages already published (all built again, each read, compared, its signature checked, the
  database made and signed, uploaded to the folder that stands in for the release). The work folder and the release are put
  back before every run (not measured).

A small C launcher (built with cc) starts each run and reports its wall time, user + system CPU and peak RSS from wait4 (the
largest single process: a tool counts when it is larger than the program), and, through ptrace stopping it as it exits, the
program's own peak memory (VmHWM). CPU includes the tools (the same calls on both sides). Two unmeasured runs of each, then
RUNS of each, alternating which goes first, at nice 19. Medians, p10-p90 of the wall time in brackets.
"""
import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import compare as C  # noqa: E402

LAUNCH_C = r"""
#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/ptrace.h>
#include <sys/resource.h>
#include <sys/wait.h>
/* launch RESULT PROGRAM [ARGS...]: writes wall_ns user_us sys_us maxrss_kb own_hwm_kb wait_status */
static long hwm(pid_t pid) {
    char p[64], line[256];
    long kb = -1;
    snprintf(p, sizeof p, "/proc/%d/status", pid);
    FILE *f = fopen(p, "r");
    if (!f) return -1;
    while (fgets(line, sizeof line, f))
        if (!strncmp(line, "VmHWM:", 6)) sscanf(line + 6, "%ld", &kb);
    fclose(f);
    return kb;
}
int main(int argc, char **argv) {
    struct timespec a, b;
    struct rusage ru;
    int st;
    long own = -1;
    if (argc < 3) return 125;
    clock_gettime(CLOCK_MONOTONIC, &a);
    pid_t pid = fork();
    if (pid < 0) return 126;
    if (pid == 0) { ptrace(PTRACE_TRACEME, 0, 0, 0); raise(SIGSTOP); execv(argv[2], argv + 2); _exit(127); }
    if (waitpid(pid, &st, 0) < 0) return 126;
    ptrace(PTRACE_SETOPTIONS, pid, 0, (void *)(long)(PTRACE_O_TRACEEXIT | PTRACE_O_TRACEEXEC));
    ptrace(PTRACE_CONT, pid, 0, 0);
    for (;;) {
        if (wait4(pid, &st, 0, &ru) < 0) return 126;
        if (WIFEXITED(st) || WIFSIGNALED(st)) break;
        if (!WIFSTOPPED(st)) continue;
        int sig = WSTOPSIG(st), ev = st >> 16;
        if (sig == SIGTRAP && ev == PTRACE_EVENT_EXIT) { own = hwm(pid); ptrace(PTRACE_CONT, pid, 0, 0); }
        else if (sig == SIGTRAP && ev) ptrace(PTRACE_CONT, pid, 0, 0);
        else ptrace(PTRACE_CONT, pid, 0, (void *)(long)sig);
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    FILE *f = fopen(argv[1], "w");
    if (!f) return 126;
    fprintf(f, "%lld %lld %lld %ld %ld %d\n", (long long)(b.tv_sec - a.tv_sec) * 1000000000LL + (b.tv_nsec - a.tv_nsec),
            (long long)ru.ru_utime.tv_sec * 1000000LL + ru.ru_utime.tv_usec,
            (long long)ru.ru_stime.tv_sec * 1000000LL + ru.ru_stime.tv_usec, ru.ru_maxrss, own, st);
    if (fclose(f) != 0) return 126;
    return WIFEXITED(st) ? WEXITSTATUS(st) : 128 + WTERMSIG(st);
}
"""

SH_GIT = """#!/bin/sh
case "$3" in
  rev-parse) read -r h < "$2/.git/parity-head"; echo "$h" ;;
  symbolic-ref) echo origin/main ;;
esac
exit 0
"""
SH_GH = """#!/bin/sh
rel=/mnt/w/release
case "$2" in
  view) printf '{"assets":['; sep=; for f in "$rel"/*; do printf '%s{"name":"%s"}' "$sep" "${f##*/}"; sep=,; done; printf ']}\\n' ;;
  create) /usr/bin/mkdir "$rel" ;;
  upload) shift 6; [ $# -gt 0 ] && /usr/bin/cp "$@" "$rel"/ ;;
  delete-asset) /usr/bin/rm -f "$rel/$4" ;;
esac
exit 0
"""
SH_BUILD = """#!/bin/sh
/usr/bin/cp /mnt/bench/iso/* repo/x86_64/
"""
SH_MAKEPKG = """#!/bin/sh
for f in /mnt/bench/store/${PWD##*/}/*; do [ -e "$f" ] && /usr/bin/cp "$f" "$PKGDEST"/; done
exit 0
"""


def write(path, text, mode=0o755):
    if os.path.lexists(path):
        os.unlink(path)
    with open(path, "w") as f:
        f.write(text)
    os.chmod(path, mode)


def setup(new, test_key):
    """A world after one publish of 30 packages, with the small stand-ins."""
    P = C.make_packages()
    wj = C.new_world(P)
    wj["test_key"] = test_key
    iso = [f"many-{i:02}" for i in range(23)] + ["keyring-1", "tools-1", "sarina-1", "voice-1", "panel-2"]
    store = {"yggdrasil-2048-git": ["g2048-1"], "yggdrasil-timer-git": ["timer-1"]}
    lab = {**C.LAB, **{f"many-{i:02}": f"parity-many-{i:02}" for i in range(23)}}
    step = C.st(1, build=iso, store=store, sources={**C.SRC, "labyrinth": lab})
    r = C.run_step([new], step, wj, False)
    assert r["code"] == 0, r["stderr"]
    os.makedirs("/mnt/bench/iso")
    for pid in iso:
        shutil.copy2(os.path.join(C.PKGS, P[pid]), "/mnt/bench/iso/")
    for recipe, ids in store.items():
        os.makedirs(f"/mnt/bench/store/{recipe}")
        for pid in ids:
            shutil.copy2(os.path.join(C.PKGS, P[pid]), f"/mnt/bench/store/{recipe}/")
    for name, head in step["heads"].items():
        with open(os.path.join(C.W, "work", "src", name, ".git", "parity-head"), "w") as f:
            f.write(head + "\n")
    b = os.path.join(C.W, "bin")
    write(os.path.join(b, "git"), SH_GIT)
    write(os.path.join(b, "gh"), SH_GH)
    write(os.path.join(b, "makepkg"), SH_MAKEPKG)
    write(os.path.join(C.W, "work", "src", "Labyrinth_OS", "scripts", "build-local-packages.sh"), SH_BUILD)
    write(os.path.join(b, "gpg"), f"""#!/bin/sh
for a; do shift; [ "$a" = {C.KEY} ] && a={test_key}; set -- "$@" "$a"; done
exec /usr/bin/gpg --faked-system-time 20261113T120000! "$@"
""")
    for t in ("bsdtar", "gzip", "vercmp", "repo-add"):
        os.unlink(os.path.join(b, t))
        os.symlink("/usr/bin/" + t, os.path.join(b, t))
    shutil.copytree(os.path.join(C.W, "work"), "/mnt/bench/work", symlinks=True)
    shutil.copytree(os.path.join(C.W, "release"), "/mnt/bench/release")


def reset():
    for d in ("work", "release"):
        shutil.rmtree(os.path.join(C.W, d))
        shutil.copytree(f"/mnt/bench/{d}", os.path.join(C.W, d), symlinks=True)


def measure(launch, prog, args):
    env = {"PATH": os.path.join(C.W, "bin"), "HOME": os.path.join(C.W, "home"), "LANG": "C.UTF-8", "TZ": "UTC",
           "LABYRINTH_PUBLISH_DIR": os.path.join(C.W, "work"), "LABYRINTH_GNUPGHOME": os.path.join(C.W, "g")}
    res = "/mnt/bench/result"
    p = subprocess.run(["/usr/bin/nice", "-n", "19", launch, res] + prog + args, env=env, cwd=env["HOME"],
                       capture_output=True)
    wall, user, sys_, maxrss, own, st = (int(x) for x in open(res).read().split())
    if p.returncode != 0:
        raise SystemExit(f"bench: {prog} {args} exited {p.returncode}: {p.stderr.decode(errors='replace')[-2000:]}")
    return wall / 1e6, (user + sys_) / 1e3, maxrss / 1024, own / 1024


def pct(xs, q):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--old", default=os.path.join(C.REPO, "scripts", "publish-repo.py"))
    ap.add_argument("--new", default=os.path.join(C.CRATE, "target", "release", "labyrinth-publish-repo"))
    ap.add_argument("--runs", type=int, default=20)
    ap.add_argument("--json")
    ap.add_argument("--inside", action="store_true", help=argparse.SUPPRESS)
    a = ap.parse_args()
    if not a.inside:
        os.execv("/usr/bin/unshare", ["unshare", "-rmnpf", "--mount-proc", "--kill-child", "--", "/usr/bin/python3", "-I",
                                      os.path.abspath(__file__), "--inside", "--old", os.path.abspath(a.old), "--new",
                                      os.path.abspath(a.new), "--runs", str(a.runs)] + (["--json", os.path.abspath(a.json)] if a.json else []))
    test_key = C.prepare()
    launch = "/mnt/bench/launch"
    os.makedirs("/mnt/bench")
    with open("/mnt/bench/launch.c", "w") as f:
        f.write(LAUNCH_C)
    subprocess.run(["/usr/bin/cc", "-O2", "-o", launch, "/mnt/bench/launch.c"], check=True)
    setup(a.new, test_key)
    old, new = ["/usr/bin/python3", a.old], [a.new]
    cases = [("help", ["--help"], a.runs, False), ("unchanged", [], a.runs, False), ("force", ["--force"], max(5, a.runs // 4), True)]
    results = {}
    for name, args, runs, needs_reset in cases:
        r = {"old": [], "new": []}
        for i in range(2 + runs):
            order = (("old", old), ("new", new)) if i % 2 == 0 else (("new", new), ("old", old))
            for side, prog in order:
                if needs_reset:
                    reset()
                m = measure(launch, prog, args)
                if i >= 2:
                    r[side].append(m)
        results[name] = r
    out = {}
    print(f"{'case':<10} {'side':<4} {'wall ms (p10-p90)':<28} {'CPU ms':>9} {'peak RSS MB':>12} {'own peak MB':>12}")
    for name, r in results.items():
        out[name] = {}
        for side in ("old", "new"):
            walls = [m[0] for m in r[side]]
            row = {"wall": statistics.median(walls), "p10": pct(walls, 0.1), "p90": pct(walls, 0.9),
                   "cpu": statistics.median(m[1] for m in r[side]), "maxrss": statistics.median(m[2] for m in r[side]),
                   "own": statistics.median(m[3] for m in r[side])}
            out[name][side] = row
            print(f"{name:<10} {side:<4} {row['wall']:9.2f} ({row['p10']:.2f}-{row['p90']:.2f}){'':<6} {row['cpu']:9.2f} "
                  f"{row['maxrss']:12.1f} {row['own']:12.1f}")
    if a.json:
        with open(a.json, "w") as f:
            json.dump(out, f, indent=1)


if __name__ == "__main__":
    main()
