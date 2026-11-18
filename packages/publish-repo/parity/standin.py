#!/usr/bin/python3 -I
"""The made-up world's programs for compare.py: one file, linked under each name (git, gh, makepkg, gpg, bsdtar, gzip,
vercmp, repo-add and the clone's scripts/build-local-packages.sh). Each writes its call down (argv, folder, the environment
variables the publish sets) in the world's calls.jsonl, then acts as the step in world.json says.

git, gh, makepkg and build-local-packages.sh are stand-ins: nothing is cloned, fetched, built or uploaded (the "release" is a
folder of the world). gpg, bsdtar, gzip, vercmp and repo-add are the real programs, run by absolute path after the call is
written down; gpg only with a GNUPGHOME inside the world (the throw-away keyring), Labyrinth's fingerprint swapped for the
throw-away key's and the clock fixed, so two runs make the same signatures. Calls made by repo-add and by the build stand-in
(PARITY_INNER) are not written down: they are the same program's own business.
"""
import json
import os
import shutil
import signal
import subprocess
import sys

WORLD = os.environ["PARITY_WORLD"]
KEY = "C9CE784C8BD9B661042075972D3BD4B1E7CD07A5"
ENV_SEEN = ("GNUPGHOME", "YGGDRASIL_DIR", "LABYRINTH_SKIP", "BUILDDIR", "SRCDEST", "PKGDEST")
REAL = {"bsdtar": "/usr/bin/bsdtar", "gzip": "/usr/bin/gzip", "vercmp": "/usr/bin/vercmp", "repo-add": "/usr/bin/repo-add",
        "gpg": "/usr/bin/gpg"}


def world():
    with open(os.path.join(WORLD, "world.json")) as f:
        return json.load(f)


def note(name, argv):
    if os.environ.get("PARITY_INNER"):
        return
    rec = {"prog": name, "argv": argv, "cwd": os.getcwd(), "env": {k: os.environ[k] for k in ENV_SEEN if k in os.environ}}
    with open(os.path.join(WORLD, "calls.jsonl"), "a") as f:
        f.write(json.dumps(rec) + "\n")


def counter(name):
    """How many times this has been called in this run (a file per run, reset by compare.py)."""
    p = os.path.join(WORLD, "count-" + name)
    n = int(open(p).read()) + 1 if os.path.exists(p) else 1
    with open(p, "w") as f:
        f.write(str(n))
    return n


def sync_tree(name, dest):
    """The checkout's committed files as the step's template has them; .git and ignored output (repo/) stay."""
    tpl = os.path.join(WORLD, "templates", name)
    os.makedirs(dest, exist_ok=True)
    for e in os.listdir(dest):
        if e in (".git", "repo"):
            continue
        p = os.path.join(dest, e)
        if os.path.isdir(p) and not os.path.islink(p):
            shutil.rmtree(p)
        else:
            os.unlink(p)
    for e in os.listdir(tpl):
        s, d = os.path.join(tpl, e), os.path.join(dest, e)
        if os.path.isdir(s) and not os.path.islink(s):
            shutil.copytree(s, d, symlinks=True)
        else:
            shutil.copy2(s, d, follow_symlinks=False)
    os.makedirs(os.path.join(dest, ".git"), exist_ok=True)


def git(argv, step):
    a = argv[1:]
    if a[:2] == ["clone", "--quiet"]:
        name = a[2].rsplit("/", 1)[1].removesuffix(".git")
        if step.get("fail") == "clone":
            sys.stderr.write("fatal: unable to access the repository\n")
            return 128
        sync_tree(name, a[3])
        return 0
    if a[0] != "-C":
        return 2
    d, a = a[1], a[2:]
    name = os.path.basename(d)
    if a[0] == "fetch":
        if step.get("fail") == "fetch":
            sys.stderr.write("fatal: unable to access 'https://github.com/...': Could not resolve host\n")
            return 128
        sync_tree(name, d)
        return 0
    if a[0] == "symbolic-ref":
        print(step.get("origin_head", {}).get(name, "origin/main"))
        return 0
    if a[0] in ("checkout", "clean"):
        return 0
    if a == ["rev-parse", "HEAD"]:
        print(step["heads"].get(name, "0" * 40))
        return 0
    return 2


def put_packages(ids, dest, step):
    os.makedirs(dest, exist_ok=True)
    pkgs = world()["packages"]
    for pid in ids:
        file = pkgs[pid]
        shutil.copy2(os.path.join(WORLD, "pkgs", file), os.path.join(dest, os.path.basename(file)))
        sig = step.get("sigs", {}).get(pid)
        target = os.path.join(dest, os.path.basename(file))
        if sig == "good":
            env = dict(os.environ, PARITY_INNER="1", GNUPGHOME=os.path.join(WORLD, "g"))
            subprocess.run([os.path.join(WORLD, "bin", "gpg"), "--batch", "--yes", "--local-user", KEY, "--detach-sign",
                            "--no-armor", "--output", target + ".sig", target], env=env, check=True, capture_output=True)
        elif sig == "stale":
            with open(target + ".sig", "wb") as f:
                f.write(b"\x88\x75not a signature of this file")


def build_local(argv, step):
    if step.get("fail") == "build":
        sys.stderr.write("==> ERROR: A failure occurred in build().\n")
        return 2
    print("==> (stand-in) building the ISO's packages")
    put_packages(step.get("build", []), os.path.join(os.getcwd(), "repo", "x86_64"), step)
    return 0


def makepkg(argv, step):
    recipe = os.path.basename(os.getcwd())
    if not os.path.isfile(argv[2]):
        sys.stderr.write("makepkg: no such config\n")
        return 1
    if step.get("fail") == "makepkg:" + recipe:
        sys.stderr.write("==> ERROR: A failure occurred in package().\n")
        return 4
    put_packages(step.get("store", {}).get(recipe, []), os.environ["PKGDEST"], step)
    return 0


def gh(argv, step):
    rel = os.path.join(WORLD, "release")
    a = argv[1:]
    if a[:2] == ["release", "view"]:
        if not os.path.isdir(rel):
            sys.stderr.write("release not found\n")
            return 1
        print(json.dumps({"assets": [{"name": n, "size": os.path.getsize(os.path.join(rel, n))} for n in sorted(os.listdir(rel))]}))
        return 0
    if a[:2] == ["release", "create"]:
        os.makedirs(rel)
        return 0
    if a[:2] == ["release", "upload"]:
        n = counter("upload")
        files = [x for x in a[2:] if x.startswith("/")]
        fail = step.get("fail") == "upload:%d" % n
        for i, f in enumerate(files):
            if fail and i == 1:
                sys.stderr.write("HTTP 502: Bad Gateway (upload)\n")
                return 1
            shutil.copy(f, os.path.join(rel, os.path.basename(f)))
        if fail:
            sys.stderr.write("HTTP 502: Bad Gateway (upload)\n")
            return 1
        return 0
    if a[:2] == ["release", "delete-asset"]:
        p = os.path.join(rel, a[3])
        if not os.path.exists(p):
            return 1
        os.unlink(p)
        return 0
    return 2


def real(name, argv, step):
    env = dict(os.environ)
    args = argv[1:]
    if name == "gpg":
        home = env.get("GNUPGHOME", "")
        if not home.startswith(WORLD + "/"):
            sys.stderr.write("stand-in gpg: GNUPGHOME outside the world, refused\n")
            return 99
        args = ["--faked-system-time", "20261113T120000!"] + [x.replace(KEY, world()["test_key"]) for x in args]
    if name == "repo-add":
        if step.get("fail") == "repo-add":
            sys.stderr.write("==> ERROR: repo-add failed (stand-in)\n")
            return 1
        env["PARITY_INNER"] = "1"
    # Python ignores SIGPIPE and exec keeps that: the real program must get it back as the publish would start it
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    os.execve(REAL[name], [REAL[name]] + args, env)


def main():
    name = os.path.basename(sys.argv[0])
    argv = [name] + sys.argv[1:]
    note(name, argv)
    w = world()
    step = w["step"]
    if name in REAL:
        return real(name, argv, step)
    return {"git": git, "gh": gh, "makepkg": makepkg, "build-local-packages.sh": build_local}[name](argv, step)


if __name__ == "__main__":
    sys.exit(main())
