#!/usr/bin/env python3
"""publish-repo.py: Labyrinth's package repository, built, signed and put online.

Every installed Labyrinth system takes Labyrinth's own software (the desktop, Sarina, Heimdall, Limbo, the store's apps) from
the [labyrinth] repository in its /etc/pacman.conf: the release named "packages" of github.com/Artifex-Technologies/Labyrinth_OS.
This script fills that release:

  1. fresh copies of Labyrinth_OS, Yggdrasil, Sarina, Heimdall and Limbo from GitHub (main as pushed, never a working tree
     with unfinished changes), side by side in the work folder;
  2. nothing more when none of them changed since the last publish (unless --force);
  3. the packages built from them: scripts/build-local-packages.sh (the ISO's packages, versions stamped with the commit
     count) and the store's own recipes (profile/airootfs/usr/share/labyrinth/pkgbuilds);
  4. of each package the newest build; a build whose files and dependencies are the same as what is already published is not
     published again (the published one stays, so nobody downloads the same program twice);
  5. each package and the database signed with Labyrinth's key (its own keyring, ~/.local/share/labyrinth-signing/gnupg; the
     backup and how to restore it are on Proton Drive, Documents/Github/Labyrinth_OS/package-signing-key);
  6. uploaded: the packages first, the database last, then the files of the publish before last are removed (the last
     publish's stay a day, for systems that read the database just before this one replaced it).

Left out on purpose: software whose licence does not allow passing it on (Proton Pass, VS Code: Agora installs those from
the AUR, downloaded from Proton and Microsoft). See docs/PACKAGE_REPO.md.

    scripts/publish-repo.py            build and publish what changed
    scripts/publish-repo.py --force    build and publish even if nothing changed
    scripts/publish-repo.py --dry-run  build and sign, show what would be uploaded, upload nothing
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ORG = "Artifex-Technologies"
GH_REPO = f"{ORG}/Labyrinth_OS"
TAG = "packages"
REPO_NAME = "labyrinth"
KEY = "C9CE784C8BD9B661042075972D3BD4B1E7CD07A5"
SOURCES = ["Labyrinth_OS", "Yggdrasil", "Sarina", "Heimdall", "Limbo"]
NOT_PUBLISHED = set()
# asset names GitHub keeps as they are (anything else it renames, and pacman would ask for a file that is not there)
SAFE_NAME = re.compile(r"^[A-Za-z0-9._-]+$")

WORK = Path(os.environ.get("LABYRINTH_PUBLISH_DIR", Path.home() / ".cache" / "labyrinth-publish"))
GNUPGHOME = os.environ.get("LABYRINTH_GNUPGHOME", str(Path.home() / ".local" / "share" / "labyrinth-signing" / "gnupg"))


def say(msg):
    print(f"==> {msg}", flush=True)


def run(cmd, **kw):
    kw.setdefault("check", True)
    return subprocess.run(cmd, **kw)


def out(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw).stdout


# --- 1, 2: sources -----------------------------------------------------------------------------------------------------

def fresh_copies(src):
    """Each source repository as it is on GitHub. Returns {name: commit}."""
    src.mkdir(parents=True, exist_ok=True)
    heads = {}
    for name in SOURCES:
        d = src / name
        if not (d / ".git").is_dir():
            run(["git", "clone", "--quiet", f"https://github.com/{ORG}/{name}.git", str(d)])
        else:
            run(["git", "-C", str(d), "fetch", "--quiet", "origin"])
        branch = out(["git", "-C", str(d), "symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).strip()
        # the local branch moves with GitHub: recipes that build from this copy (git+file://, heimdall-git, sarina-git,
        # limbo-git) take its branch, not the checkout, so a detached checkout left them building the first clone's
        # commit for days (2026-11-07: Heimdall two days behind, Sarina too)
        run(["git", "-C", str(d), "checkout", "--quiet", "--force", "-B", branch.split("/", 1)[1], branch])
        # tracked files exactly as committed; ignored build output (the local repo, makepkg's folders) is kept
        run(["git", "-C", str(d), "clean", "--quiet", "-ffd"])
        heads[name] = out(["git", "-C", str(d), "rev-parse", "HEAD"]).strip()
    return heads


# --- 3: building -------------------------------------------------------------------------------------------------------

def build(src):
    labyrinth = src / "Labyrinth_OS"
    env = dict(os.environ, YGGDRASIL_DIR=str(src / "Yggdrasil"), LABYRINTH_SKIP=" ".join(sorted(NOT_PUBLISHED)))
    say("building the ISO's packages (scripts/build-local-packages.sh)")
    run([str(labyrinth / "scripts" / "build-local-packages.sh")], cwd=labyrinth, env=env)
    repo_dir = labyrinth / "repo" / "x86_64"
    # the store's own recipes (they clone from GitHub); those also built above from a local checkout are not built twice
    built_above = {p.name for p in (labyrinth / "packages" / "pkgbuilds").iterdir()} | {p.name for p in (src / "Yggdrasil" / "packages" / "pkgbuilds").iterdir()}
    store = labyrinth / "profile" / "airootfs" / "usr" / "share" / "labyrinth" / "pkgbuilds"
    for d in sorted(store.iterdir()):
        if not (d / "PKGBUILD").is_file() or d.name in built_above or d.name in NOT_PUBLISHED:
            continue
        say(f"building the store's {d.name}")
        # always the same folder: a build path ends up inside the programs, and a new random one each time would make every
        # build look changed (and be published again for nothing)
        work = WORK / "store" / d.name
        if work.exists():
            shutil.rmtree(work)
        shutil.copytree(d, work)
        run(["makepkg", "-f", "-s", "-c", "--noconfirm"], cwd=work)
        for f in work.glob("*.pkg.tar.zst"):
            shutil.copy2(f, repo_dir / f.name)
    return repo_dir


# --- 4: choosing what to publish --------------------------------------------------------------------------------------

def pkginfo(path):
    """The package's .PKGINFO as (name, version, lines)."""
    text = out(["bsdtar", "-xOf", str(path), ".PKGINFO"])
    fields = {}
    lines = []
    for l in text.splitlines():
        if " = " in l:
            k, v = l.split(" = ", 1)
            fields.setdefault(k, v)
            lines.append((k, v))
    return fields["pkgname"], fields["pkgver"], lines


def vercmp(a, b):
    return int(out(["vercmp", a, b]).strip())


def content(path):
    """What makes a package different to the person installing it: its files' digests (from its .MTREE), its install script
    and what it depends on, provides, conflicts with and replaces. Not its version, build date or size."""
    mtree = subprocess.run(["bsdtar", "-xOf", str(path), ".MTREE"], check=True, capture_output=True).stdout
    mtree = subprocess.run(["gzip", "-dc"], input=mtree, check=True, capture_output=True).stdout.decode()
    files = []
    for l in mtree.splitlines():
        parts = l.split()
        if not parts or parts[0].startswith("/") or parts[0] in ("./.PKGINFO", "./.BUILDINFO", "./.MTREE"):
            continue
        keep = [p for p in parts[1:] if p.startswith(("sha256digest=", "mode=", "type=", "link="))]
        files.append(parts[0] + " " + " ".join(sorted(keep)))
    _, _, lines = pkginfo(path)
    meta = [f"{k}={v}" for k, v in lines if k in ("depend", "optdepend", "provides", "conflict", "replaces", "backup", "arch")]
    return hashlib.sha256("\n".join(sorted(files) + sorted(meta)).encode()).hexdigest()


def choose(repo_dir, published_dir):
    """{name: path} of what this publish holds: of each package the newest build, or the published one when the newest build
    is the same program."""
    newest = {}
    for f in repo_dir.glob("*.pkg.tar.zst"):
        name, ver, _ = pkginfo(f)
        if name in NOT_PUBLISHED or name.startswith("labyrinth-") and name not in allowed_labyrinth(repo_dir):
            continue
        if name not in newest or vercmp(ver, newest[name][0]) > 0:
            newest[name] = (ver, f)
    published = {}
    for f in published_dir.glob("*.pkg.tar.zst") if published_dir.is_dir() else []:
        name, ver, _ = pkginfo(f)
        published[name] = (ver, f)
    chosen = {}
    for name, (ver, f) in sorted(newest.items()):
        old = published.get(name)
        if old and (vercmp(ver, old[0]) <= 0 or content(f) == content(old[1])):
            chosen[name] = old[1]
        else:
            chosen[name] = f
    return chosen


_allowed = None


def allowed_labyrinth(repo_dir):
    """The labyrinth-* packages that still have a recipe (older names such as labyrinth-panel became yggdrasil-*)."""
    global _allowed
    if _allowed is None:
        root = repo_dir.parent.parent
        _allowed = {p.name for p in (root / "packages" / "pkgbuilds").iterdir() if p.name.startswith("labyrinth-")}
    return _allowed


# --- 5: signing --------------------------------------------------------------------------------------------------------

def stage(chosen, stage_dir):
    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    stage_dir.mkdir(parents=True)
    env = dict(os.environ, GNUPGHOME=GNUPGHOME)
    for name, f in chosen.items():
        if not SAFE_NAME.match(f.name):
            sys.exit(f"error: {f.name}: GitHub would rename this file; pacman could not find it")
        shutil.copy2(f, stage_dir / f.name)
        sig = f.with_name(f.name + ".sig")
        if sig.is_file() and f.parent != stage_dir:
            shutil.copy2(sig, stage_dir / sig.name)
        if not (stage_dir / (f.name + ".sig")).is_file():
            run(["gpg", "--batch", "--yes", "--local-user", KEY, "--detach-sign", "--no-armor", "--output", str(stage_dir / (f.name + ".sig")), str(stage_dir / f.name)], env=env)
        run(["gpg", "--batch", "--verify", str(stage_dir / (f.name + ".sig")), str(stage_dir / f.name)], env=env, capture_output=True)
    db = stage_dir / f"{REPO_NAME}.db.tar.gz"
    run(["repo-add", "--quiet", "--sign", "--key", KEY, str(db)] + sorted(str(stage_dir / f.name) for f in chosen.values()), env=env)
    # pacman asks for labyrinth.db and labyrinth.files: repo-add makes those as links, and a release holds files only
    for base in (f"{REPO_NAME}.db", f"{REPO_NAME}.files"):
        for ext in ("", ".sig"):
            link = stage_dir / (base + ext)
            target = stage_dir / (base + ".tar.gz" + ext)
            if link.is_symlink():
                link.unlink()
            shutil.copy2(target, link)
            target.unlink()
    for old in stage_dir.glob("*.old*"):
        old.unlink()


# --- 6: uploading ------------------------------------------------------------------------------------------------------

def release_assets():
    try:
        j = json.loads(out(["gh", "release", "view", TAG, "-R", GH_REPO, "--json", "assets"]))
        return {a["name"] for a in j["assets"]}
    except subprocess.CalledProcessError:
        say(f"making the release {TAG}")
        notes = ("Labyrinth OS's package repository: Labyrinth's own software, signed with the Labyrinth key "
                 f"({KEY}, in the labyrinth-keyring package). Installed systems use it through [labyrinth] in "
                 "/etc/pacman.conf. Not meant for downloading by hand. Filled by scripts/publish-repo.py.")
        run(["gh", "release", "create", TAG, "-R", GH_REPO, "--title", "Package repository", "--notes", notes, "--latest=false"])
        return set()


def upload(stage_dir, state):
    have = release_assets()
    files = sorted(p for p in stage_dir.iterdir() if p.is_file())
    db_files = [p for p in files if p.name.startswith(f"{REPO_NAME}.db") or p.name.startswith(f"{REPO_NAME}.files")]
    pkgs = [p for p in files if p not in db_files]
    new = [p for p in pkgs if p.name not in have]
    say(f"uploading {len(new)} new file(s) of {len(pkgs)}")
    for i in range(0, len(new), 20):
        run(["gh", "release", "upload", TAG, "-R", GH_REPO, "--clobber"] + [str(p) for p in new[i:i + 20]])
    say("uploading the database")
    run(["gh", "release", "upload", TAG, "-R", GH_REPO, "--clobber"] + [str(p) for p in db_files])
    # what neither this publish nor the one before it holds goes
    previous = set(json.loads(state.read_text())["files"]) if state.is_file() else set()
    current = {p.name for p in files}
    for name in sorted(have - current - previous):
        run(["gh", "release", "delete-asset", TAG, name, "-R", GH_REPO, "--yes"])
    return current


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--force", action="store_true", help="publish even when no source changed")
    ap.add_argument("--dry-run", action="store_true", help="build and sign, upload nothing")
    a = ap.parse_args()

    env = dict(os.environ, GNUPGHOME=GNUPGHOME)
    if subprocess.run(["gpg", "--batch", "--list-secret-keys", KEY], env=env, capture_output=True).returncode != 0:
        sys.exit(f"error: the signing key {KEY} is not in {GNUPGHOME} (see the README next to its backup on Proton Drive)")

    WORK.mkdir(parents=True, exist_ok=True)
    state = WORK / "published.json"
    src = WORK / "src"
    heads = fresh_copies(src)
    last = json.loads(state.read_text()) if state.is_file() else {}
    if last.get("heads") == heads and not a.force:
        say("nothing changed on GitHub since the last publish")
        return
    repo_dir = build(src)
    published_dir = WORK / "published"
    chosen = choose(repo_dir, published_dir)
    stage_dir = WORK / "stage"
    stage(chosen, stage_dir)
    changed = sorted(n for n, f in chosen.items() if f.parent != published_dir)
    say(f"{len(chosen)} packages, {len(changed)} new or changed: {', '.join(changed) or 'none'}")
    if a.dry_run:
        say(f"dry run: nothing uploaded; the signed repository is in {stage_dir}")
        return
    files = upload(stage_dir, state)
    # what is published now becomes what the next publish compares against
    if published_dir.exists():
        shutil.rmtree(published_dir)
    shutil.copytree(stage_dir, published_dir)
    state.write_text(json.dumps({"heads": heads, "files": sorted(files)}, indent=1) + "\n")
    say(f"published: https://github.com/{GH_REPO}/releases/tag/{TAG}")


if __name__ == "__main__":
    main()
