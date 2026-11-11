"""The cases of the shell scripts' ports that run as root (loaded by compare.py): labyrinth-rollback, labyrinth-install-bootfinish,
labyrinth-install-cleanup (and, in cases_root2.py, the other root ones, on the same world).

Each run is in a world of its own: a user and mount namespace (unshare, no real root) where uid 0 is this account and uids
1000-2999 are its subordinate ids (/etc/subuid), with a root folder of its own (WORLD/root, chroot). /, /etc, /var, /run, /boot,
/home, /tmp and /dev are the world's (its /dev has null, zero, urandom, the fd links and fakedisk0/fakedisk1, which are this
computer's first block device bound in so that [[ -b ]] holds; nothing opens them), /usr is this computer's with /usr/bin,
/usr/share and /usr/lib laid over (overlays, so nothing written there reaches this computer) and the folders a case owns
(/usr/lib/systemd/user ...) bound over them, /usr/local is the world's, /proc is this computer's. The program, script or Rust,
is copied into the world and run there.

PATH is the world's stand-ins (WORLD/bin), then WORLD/sysbin: links to the few real tools the scripts use (bash, sed, grep, cp
...), so that a program a case does not give is not there (`command -v`). Every stand-in writes down its arguments and answers
from fake/NAME_ARG1_ARG2... (the longest run of its arguments that has an answer; '/' written as '%'): .out, .err, .rc, .sh (run
in the stand-in: the mount of a partition binds a folder of the world, btrfs's snapshot copies one) and .stdin (what it read).

The partitions are folders of the world (WORLD/disk/fakedisk0 ...); a subvolume is a folder holding .subvol.

Nothing on this computer is changed and none of the real programs that would change it runs."""
import os
import stat

import __main__ as P

W = P.WORLD
R = os.path.join(W, "root")
S = P.SCRATCH


def subordinate(path):
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
MAPPED = SUBUID is not None and SUBGID is not None
UNSHARE = (["/usr/bin/unshare", "--map-root-user", "--map-users=1000:%s:2000" % SUBUID, "--map-groups=1000:%s:2000" % SUBGID, "-m", "-u"]
           if MAPPED else ["/usr/bin/unshare", "-r", "-m", "-u"])
WRAP = UNSHARE + ["--", "/bin/sh", os.path.join(W, "ns.sh")]


def first_block_device():
    for e in sorted(os.scandir("/dev"), key=lambda e: e.name):
        try:
            if stat.S_ISBLK(os.stat(e.path).st_mode):
                return e.path
        except OSError:
            pass
    return None


BLOCK = first_block_device()

SYSBIN = ("bash sh env sed grep awk gawk tail head cat cut tr date mktemp rm mv cp ls sort diff rmdir install mkdir chmod chown ln df sync "
          "id touch basename dirname xargs sha256sum wc printf sleep seq python3 setsid stat readlink realpath getent tee test true false "
          "uname find").split()

FAKE = r"""#!/bin/sh
W=@W@
n=${0##*/}
line=$n
for a in "$@"; do line="$line [$a]"; done
printf '%s\n' "$line" >> "$W/calls"
has() { [ -e "$1.sh" ] || [ -e "$1.out" ] || [ -e "$1.err" ] || [ -e "$1.rc" ] || [ -e "$1.stdin" ]; }
k=$W/fake/$n
best=
has "$k" && best=$k
for a in "$@"; do
  k="${k}_$(printf '%s' "$a" | tr '/' '%')"
  has "$k" && best=$k
done
if [ -n "$best" ]; then
  if [ -e "$best.stdin" ]; then printf 'stdin: ' >> "$W/calls"; cat >> "$W/calls"; echo '<end>' >> "$W/calls"; fi
  [ -e "$best.sh" ] && . "$best.sh"
  [ -e "$best.out" ] && cat "$best.out"
  [ -e "$best.err" ] && cat "$best.err" >&2
  [ -e "$best.rc" ] && exit "$(cat "$best.rc")"
fi
exit 0
"""

NS = r"""#!/bin/sh
# the world's root (see cases_root.py), then the program, copied in, run in it
set -e
trap 'echo "parity-ns: the world could not be made" >&2' EXIT
W=@W@
R=$W/root
/usr/bin/hostname parity-host
/usr/bin/mount --rbind /usr "$R/usr"
for d in /usr/bin /usr/share /usr/lib; do
  /usr/bin/mkdir -p "$W/ov$d/up" "$W/ov$d/work"
  while read -r name; do [ -n "$name" ] && /usr/bin/mknod "$W/ov$d/up/$name" c 0 0; done < "$W/whiteouts$(echo "$d" | /usr/bin/tr / -)"
  /usr/bin/mount -t overlay overlay -o "lowerdir=$d,upperdir=$W/ov$d/up,workdir=$W/ov$d/work,userxattr" "$R$d"
done
/usr/bin/mount --bind "$W/usrlocal" "$R/usr/local"
while read -r own; do
  [ -n "$own" ] || continue
  /usr/bin/mkdir -p "$R$own"
  /usr/bin/mount --bind "$W/own$own" "$R$own"
done < "$W/own.list"
for n in null zero full random urandom tty; do /usr/bin/touch "$R/dev/$n"; /usr/bin/mount --bind "/dev/$n" "$R/dev/$n"; done
for n in fakedisk0 fakedisk1; do /usr/bin/touch "$R/dev/$n"; /usr/bin/mount --bind "@BLOCK@" "$R/dev/$n"; done
/usr/bin/mount --rbind /proc "$R/proc"
/usr/bin/mkdir -p "$R@S@"
/usr/bin/mount --bind "@S@" "$R@S@"
p=$1
shift
/usr/bin/cp "$p" "$W/ov/prog"
/usr/bin/chmod 755 "$W/ov/prog"
cd /
trap - EXIT
export PATH="$INNER_PATH"
unset INNER_PATH
exec /usr/bin/chroot "$R" "$W/ov/prog" "$@"
"""


def host(path):
    """Where a path of the world's root is on this computer."""
    if path == "/usr/local" or path.startswith("/usr/local/"):
        return os.path.join(W, "usrlocal", path[len("/usr/local/"):])
    if path.startswith("/usr/bin/"):
        return os.path.join(W, "ov", "usr", "bin", "up", path[len("/usr/bin/"):])
    if path.startswith("/usr/"):
        return os.path.join(W, "own", path[1:])
    return R + path


def put(path, v):
    """A file of the world: text or bytes (mode 0644), (text, mode), ("link", target) or ("dir", mode)."""
    p = host(path)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    if isinstance(v, tuple) and v[0] == "link":
        os.symlink(v[1], p)
    elif isinstance(v, tuple) and v[0] == "dir":
        os.makedirs(p, exist_ok=True)
        os.chmod(p, v[1])
    elif isinstance(v, tuple):
        P.write(p, v[0], v[1])
    else:
        P.write(p, v, 0o644)


def fake(key, out=None, rc=None, err=None, sh=None, stdin=False):
    """The answer of a stand-in to NAME_ARG1_ARG2... (see the module's text)."""
    k = os.path.join(W, "fake", key.replace("/", "%"))
    for ext, v in ((".out", out), (".rc", None if rc is None else str(rc)), (".err", err), (".sh", sh), (".stdin", "" if stdin else None)):
        if v is not None:
            P.write(k + ext, v)


PASSWD = "root:x:0:0::/root:/bin/bash\n"
GROUP = "root:x:0:\nwheel:x:998:\n"


def world(files=None, stubs=(), fakes=(), own=(), whiteouts=None, passwd="", group="", disks=None, extra=None):
    """A case's world. files: {path: see put}; stubs: the stand-ins' names; fakes: [(key, {out, rc, err, sh, stdin})]; own: the
    folders under /usr that are the world's (empty unless files puts something there); whiteouts: {"/usr/bin": [names]} hidden
    from this computer's; disks: {device name: {relative path: see put}} (a partition's top level)."""
    def go(paths):
        for d in ("etc", "var/log", "var/lib", "var/cache", "run", "boot", "home", "root", "usr", "proc", "dev", "tmp", "mnt"):
            os.makedirs(os.path.join(R, d), exist_ok=True)
        os.chmod(os.path.join(R, "tmp"), 0o1777)
        for n, t in (("bin", "usr/bin"), ("sbin", "usr/bin"), ("lib", "usr/lib"), ("lib64", "usr/lib")):
            os.symlink(t, os.path.join(R, n))
        for n, t in (("fd", "/proc/self/fd"), ("stdin", "/proc/self/fd/0"), ("stdout", "/proc/self/fd/1"), ("stderr", "/proc/self/fd/2")):
            os.symlink(t, os.path.join(R, "dev", n))
        P.write(os.path.join(R, "etc", "passwd"), PASSWD + passwd)
        P.write(os.path.join(R, "etc", "group"), GROUP + group)
        P.write(os.path.join(R, "etc", "nsswitch.conf"), "passwd: files\ngroup: files\nshadow: files\n")
        os.makedirs(os.path.join(W, "usrlocal"), exist_ok=True)
        for d in own:
            os.makedirs(os.path.join(W, "own", d[1:]), exist_ok=True)
        for path, v in (files or {}).items():
            if v is not None:
                put(path, v)
        owned = sorted({"/" + os.path.relpath(d, os.path.join(W, "own")) for d in _own_roots()})
        P.write(os.path.join(W, "own.list"), "".join(d + "\n" for d in owned))
        for d in ("/usr/bin", "/usr/share", "/usr/lib"):
            P.write(os.path.join(W, "whiteouts" + d.replace("/", "-")), "".join(n + "\n" for n in (whiteouts or {}).get(d, ())))
        os.makedirs(os.path.join(W, "ov", "usr", "bin", "up"), exist_ok=True)
        os.makedirs(os.path.join(W, "bin"), exist_ok=True)
        for n in stubs:
            P.write(os.path.join(W, "bin", n), FAKE.replace("@W@", W), 0o755)
        os.makedirs(os.path.join(W, "sysbin"), exist_ok=True)
        for n in SYSBIN:
            if os.path.exists("/usr/bin/" + n) and not os.path.exists(os.path.join(W, "bin", n)):
                os.symlink("/usr/bin/" + n, os.path.join(W, "sysbin", n))
        os.makedirs(os.path.join(W, "fake"), exist_ok=True)
        for key, kw in fakes:
            fake(key, **kw)
        for dev, tree in (disks or {}).items():
            top = os.path.join(W, "disk", dev)
            os.makedirs(top, exist_ok=True)
            for rel, v in tree.items():
                p = os.path.join(top, rel)
                os.makedirs(os.path.dirname(p), exist_ok=True)
                if isinstance(v, tuple) and v[0] == "dir":
                    os.makedirs(p, exist_ok=True)
                elif isinstance(v, tuple):
                    P.write(p, v[0], v[1])
                else:
                    P.write(p, v)
        if extra:
            extra()
        P.write(os.path.join(W, "ns.sh"), NS.replace("@W@", W).replace("@S@", S).replace("@BLOCK@", BLOCK or "/dev/null"), 0o755)
    return go


def _own_roots():
    """The top folders of WORLD/own that hold a case's own folders: every folder whose parent exists on this computer's /usr
    (the mount point must be there) is bound whole."""
    base = os.path.join(W, "own")
    out = []
    for d, dirs, _ in os.walk(base):
        rel = "/" + os.path.relpath(d, base)
        if rel == "/.":
            continue
        # bind the deepest folder that is still a folder of this computer's /usr, whole
        parent = os.path.dirname(rel)
        if os.path.isdir(parent) and parent.startswith("/usr/") and d != base:
            out.append(d)
            dirs[:] = []
    return out


def owners():
    """Who owns each file of the world (as the namespace sees it: 0 is root, 1000-2999 the mapped ids)."""
    out = {}
    for base in (R, os.path.join(W, "usrlocal"), os.path.join(W, "own"), os.path.join(W, "disk")):
        for d, dirs, files in os.walk(base):
            if d.startswith(os.path.join(R, "proc")) or d.startswith(os.path.join(R, "dev")) or S in d[len(R):]:
                dirs[:] = []
                continue
            for n in dirs + files:
                st = os.lstat(os.path.join(d, n))
                out[os.path.relpath(os.path.join(d, n), W)] = "%s:%s" % (inside(st.st_uid, os.getuid(), SUBUID), inside(st.st_gid, os.getgid(), SUBGID))
    return out


def inside(i, me, sub):
    if i == me:
        return 0
    if sub is not None and sub <= i < sub + 2000:
        return 1000 + i - sub
    return "?"


def snap():
    return {"owners": owners()}


def teardown():
    """What a case gave the mapped ids, given back to this account."""
    import subprocess
    if MAPPED:
        subprocess.run(UNSHARE + ["--", "/usr/bin/chown", "-R", "-h", "0:0", W], check=False)


def norm(t):
    import re
    t = re.sub(r"/run/labyrinth-rollback(-probe)?\.[A-Za-z0-9]{6}", r"/run/labyrinth-rollback\1.<TMP>", t)
    t = re.sub(r"\d{4}-\d\d-\d\d-\d{6}", "<STAMP>", t)
    t = re.sub(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d[+-]\d\d:\d\d", "<ISOTIME>", t)
    return t


# PATH is unshare's (it starts newuidmap); ns.sh gives the program INNER_PATH
ENV = {"PATH": "/usr/bin:/bin", "INNER_PATH": W + "/bin:" + W + "/sysbin", "HOME": "/root", "USER": "root", "LANG": "C.UTF-8"}


def wrap_ok(results, calls, tree):
    """The world was made (else both sides fail alike and would look the same)."""
    return ["the namespace or chroot failed: %r" % r.get("err") for r in results if "parity-ns:" in r.get("err", "") or "unshare:" in r.get("err", "")]


def case(prog, name, steps, setup, env=None, differs=(), norm_more=None):
    e = dict(ENV)
    e.update(env or {})
    full = (lambda t: norm_more(norm(t))) if norm_more else norm
    P.case(prog, name, [dict(s, wrap=WRAP) for s in steps], setup=setup, env=e, differs=differs, norm=full, snap=snap, teardown=teardown, bare=True,
           check=wrap_ok)


# ---------------------------------------------------------------- labyrinth-rollback

RB = "labyrinth-rollback"


def info(n, date, desc, kind="single"):
    return ('<?xml version="1.0"?>\n<snapshot>\n  <type>%s</type>\n  <num>%s</num>\n  <date>%s</date>\n  <description>%s</description>\n'
            '</snapshot>\n' % (kind, n, date, desc))


def layout(snaps=((1, "2026-10-26 22:00:01", "timeline", "grub-now"), (2, "2026-10-26 23:30:00", "before NVIDIA drivers", "grub-old")),
           grub_now="grub-now", extra=None):
    """A partition's top level as Labyrinth lays it out: @ (with .snapshots), @home; each snapshot (number, date, description,
    its GRUB module)."""
    t = {"@/.subvol": "", "@/etc/marker": "current\n", "@/boot/grub/x86_64-efi/normal.mod": grub_now + "\n", "@/.snapshots/.subvol": "",
         "@home/.subvol": "", "@home/user/vault.md": "Sarina lives here\n"}
    for n, date, desc, grub in snaps:
        s = "@/.snapshots/%s/" % n
        t[s + "snapshot/.subvol"] = ""
        t[s + "snapshot/etc/marker"] = "snap%s\n" % n
        t[s + "snapshot/.snapshots"] = ("dir", 0o755)
        if grub is not None:
            t[s + "snapshot/boot/grub/x86_64-efi/normal.mod"] = grub + "\n"
        if date is not None:
            t[s + "info.xml"] = info(n, date, desc)
    t.update(extra or {})
    return t


MOUNT_SH = r"""top=$W/disk/${3##*/}
[ -d "$top" ] || { echo "mount: $3: can't read superblock" >&2; exit 32; }
case $2 in ro,*) /usr/bin/mount --bind -o ro "$top" "$4" ;; *) /usr/bin/mount --bind "$top" "$4" ;; esac || exit 32
exit 0
"""
UMOUNT_SH = 'if [ "$1" = -l ]; then /usr/bin/umount -l "$2"; else /usr/bin/umount "$1"; fi; exit $?\n'
BTRFS_SHOW_SH = '[ -e "$3/.subvol" ] || exit 1\nexit 0\n'
BTRFS_SNAP_SH = '/usr/bin/cp -a "$3" "$4" || exit 1\nexit 0\n'

RB_STUBS = ("mount", "umount", "btrfs", "findmnt", "lsblk", "logger")
RB_FAKES = [("mount_-o", {"sh": MOUNT_SH}), ("umount", {"sh": UMOUNT_SH}), ("btrfs_subvolume_show", {"sh": BTRFS_SHOW_SH}),
            ("btrfs_subvolume_snapshot", {"sh": BTRFS_SNAP_SH})]


def rb_world(disk=None, disks=None, fakes=(), files=None):
    return world(files=files, stubs=RB_STUBS, fakes=RB_FAKES + list(fakes), disks=disks or {"fakedisk0": layout() if disk is None else disk})


def rb(name, steps, **k):
    differs = k.pop("differs", ())
    case(RB, name, steps, rb_world(**k), differs=differs)


DEV = ["--device", "/dev/fakedisk0"]
rb("help", [{"args": ["-h"]}, {"args": ["--help"]}, {"args": ["list", "-h"]}])
rb("usage errors", [{"args": []}, {"args": ["bogus"]}, {"args": ["list", "status"]}, {"args": ["1", "2"]}, {"args": ["list", "3"]},
                    {"args": ["-1"]}, {"args": ["1x"]}, {"args": ["١"]}, {"args": ["1\n"]}, {"args": ["--yes"]}, {"args": ["--dry-run", "--yes"]}])
rb("--device without its value (fixed: usage)", [{"args": ["list", "--device"]}])
rb("list", [{"args": ["list"] + DEV}, {"args": ["list", "--device=/dev/fakedisk0"]}])
rb("list: odd snapshots", [{"args": ["list"] + DEV}], disk=layout(snaps=(
    (10, "2026-11-01 01:02:03", "ten", "g"), (9, "2026-1-2 3:04:05", "one-digit fields", "g"), (3, "2026-02-30 10:00:00", "no such day", "g"),
    (4, "garbage", "a <b>tag</b> <i>x</i>", "g"), (5, None, None, "g"), (7, "2026-10-26", "date only", "g"), (8, "26-10-26 22:00:01.5", "short year", "g"),
    (11, "", "", "g"), (12, "2026-10-26 22:00", "é ü — long description with spaces", "g"))))
rb("list: names that are not snapshots", [{"args": ["list"] + DEV}],
   disk=layout(extra={"@/.snapshots/notanumber/snapshot/.subvol": "", "@/.snapshots/6/info.xml": info(6, "2026-10-26 22:00:01", "no snapshot folder"),
                      "@/.snapshots/007/snapshot/.subvol": "", "@/.snapshots/007/info.xml": info(7, "2026-10-27 22:00:01", "leading zeros"),
                      "@/.snapshots/.hidden/snapshot/.subvol": ""}))
rb("list: kinds and odd info.xml", [{"args": ["list"] + DEV}], disk=layout(extra={
    "@/.snapshots/20/snapshot/.subvol": "", "@/.snapshots/20/info.xml": "<type>pre</type><date>2026-10-26 22:00:01</date>\n<description>one line</description>",
    "@/.snapshots/21/snapshot/.subvol": "", "@/.snapshots/21/info.xml": b"<description>\xff bad</description>\n<description>good</description>\n<date>x</date>\n",
    "@/.snapshots/22/snapshot/.subvol": "", "@/.snapshots/22/info.xml": "<date>2026-10-26 22:00:01</date><date>2026-10-27 22:00:01</date>\n<type>post</type>\n"}))
rb("list: no snapshots", [{"args": ["list"] + DEV}], disk={"@/.subvol": "", "@home/.subvol": ""})
rb("list: not Labyrinth's layout", [{"args": ["list"] + DEV}], disk={"other/.subvol": ""})
rb("list: not a partition", [{"args": ["list", "--device", "/dev/null"]}, {"args": ["list", "--device", "/nonexistent"]}])
rb("list: the mount fails", [{"args": ["list"] + DEV}], fakes=[("mount_-o_ro,subvolid=5_%dev%fakedisk0", {"rc": 32, "err": "mount: no\n"})])
rb("status: not rolled back", [{"args": ["status"] + DEV}])
rb("status: rolled back, systems kept", [{"args": ["status"] + DEV}], disk=layout(extra={
    "@/.labyrinth-rollback": "from snapshot 2 (x, y) at 2026-11-01 10:00:00; previous system: @broken-2026-11-01-100000\nsecond line\n\n",
    "@broken-2026-11-01-100000/.subvol": "", "@broken-2026-10-01-100000/.subvol": "", "@rolled-back-2026-11-02-090000/.subvol": "",
    "@broken-file": "not a folder\n"}))
rb("dry run: the plan, nothing changed", [{"args": ["2", "--dry-run"] + DEV}, {"args": ["1", "--dry-run"] + DEV}, {"args": ["--dry-run", "2", "--device=/dev/fakedisk0"]}])
rb("dry run: no such snapshot", [{"args": ["9", "--dry-run"] + DEV}])
rb("dry run: no date, no description", [{"args": ["5", "--dry-run"] + DEV}], disk=layout(snaps=((5, None, None, "grub-now"),)))
rb("rollback: not confirmed without a terminal", [{"args": ["2"] + DEV}])
rb("rollback to 2 (GRUB carried over), then undo", [{"args": ["2", "--yes"] + DEV}, {"args": ["status"] + DEV}, {"args": ["list"] + DEV},
                                                   {"args": ["undo", "-y"] + DEV}, {"args": ["status"] + DEV}])
rb("rollback to 1 (same GRUB)", [{"args": ["1", "--yes"] + DEV}])
rb("rollback: GRUB folder missing in the snapshot", [{"args": ["1", "--yes"] + DEV}], disk=layout(snaps=((1, "2026-10-26 22:00:01", "d", None),)))
rb("rollback: the copy fails, the system is put back", [{"args": ["2", "--yes"] + DEV}],
   fakes=[("btrfs_subvolume_snapshot", {"rc": 1, "err": "ERROR: cannot snapshot\n"})])
rb("rollback: .snapshots in the copy is a subvolume", [{"args": ["2", "--yes"] + DEV}], disk=layout(extra={"@/.snapshots/2/snapshot/.snapshots/.subvol": ""}))
rb("rollback: .snapshots in the copy is not empty", [{"args": ["2", "--yes"] + DEV}], disk=layout(extra={"@/.snapshots/2/snapshot/.snapshots/x": "x"}))
rb("undo: nothing to put back", [{"args": ["undo", "--yes"] + DEV}, {"args": ["undo", "--dry-run"] + DEV}])
rb("undo: the newest of two", [{"args": ["undo", "--dry-run"] + DEV}, {"args": ["undo", "--yes"] + DEV}],
   disk=layout(extra={"@broken-2026-11-01-100000/.subvol": "", "@broken-2026-11-01-100000/etc/marker": "older\n",
                      "@broken-2026-11-03-100000/.subvol": "", "@broken-2026-11-03-100000/etc/marker": "newer\n",
                      "@broken-2026-11-03-100000/boot/grub/x86_64-efi/normal.mod": "grub-old\n"}))
rb("find the partition: /home is btrfs", [{"args": ["list"]}], fakes=[("findmnt_-no_SOURCE_%home", {"out": "/dev/fakedisk0[/@home]\n"}),
                                                                     ("findmnt_-no_FSTYPE_%home", {"out": "btrfs\n"})])
rb("find the partition: / is btrfs", [{"args": ["status"]}], fakes=[("findmnt_-no_SOURCE_%", {"out": "/dev/fakedisk0[/@]\n"}),
                                                                   ("findmnt_-no_FSTYPE_%", {"out": "btrfs\n"}), ("findmnt_-no_FSTYPE_%home", {"out": "ext4\n"})])
rb("find the partition: the one with snapshots (USB stick)", [{"args": ["list"]}],
   fakes=[("lsblk", {"out": "/dev/fakedisk0 btrfs\n/dev/fakedisk1 ext4\n/dev/sdz9 btrfs\n/dev/loop9 \n/dev/x\n/dev/last btrfs"})])
rb("find the partition: two with snapshots", [{"args": ["list"]}], disks={"fakedisk0": layout(), "fakedisk1": layout()},
   fakes=[("lsblk", {"out": "/dev/fakedisk0 btrfs\n/dev/fakedisk1 btrfs\n"})])
rb("find the partition: none", [{"args": ["status"]}], fakes=[("lsblk", {"out": "/dev/fakedisk1 vfat\n"})])

# ---------------------------------------------------------------- labyrinth-install-bootfinish

BF = "labyrinth-install-bootfinish"
GRUB_CFG = "### BEGIN /etc/grub.d/10_linux ###\nmenuentry 'Labyrinth OS' --class labyrinth {\n\tlinux /boot/vmlinuz-linux-cachyos\n\tinitrd /boot/initramfs.img\n}\n  menuentry 'LTS' {\n}\n"
DEFAULT_GRUB = ('GRUB_DEFAULT=0\nGRUB_TIMEOUT=5\nGRUB_DISTRIBUTOR="Labyrinth"\nGRUB_CMDLINE_LINUX_DEFAULT="loglevel=3 quiet"\nGRUB_CMDLINE_LINUX=""\n'
                '#GRUB_DISABLE_OS_PROBER=false\n')
UFW_DEFAULT = 'IPV6=yes\nDEFAULT_INPUT_POLICY="ACCEPT"\nDEFAULT_OUTPUT_POLICY="DROP"\nDEFAULT_FORWARD_POLICY="DROP"\n'
BF_STUBS = ("systemctl", "pacman", "findmnt", "btrfs", "snapper", "grub-mkconfig", "rsvg-convert", "ufw", "heimdall-enable-defaults",
            "labyrinth-hardware-setup", "grub-btrfsd")
BTRFS_ROOT = [("findmnt_-no_FSTYPE_%", {"out": "btrfs\n"}), ("findmnt_-no_UUID_%", {"out": "0b5e8f9c-1d2e-4f3a-9b8c-7d6e5f4a3b2c\n"}),
              ("btrfs_inspect-internal_map-swapfile", {"out": "533760\n"}),
              ("btrfs_filesystem_mkswapfile", {"sh": 'mkdir -p /swap && echo swap > /swap/swapfile\n'}),
              ("grub-mkconfig_-o", {"sh": 'cat /boot/grub/grub.cfg /etc/default/grub > "$2"\n'})]
THEME_STUB = '#!/bin/sh\necho "theme: $*" >> @W@/calls\necho installed\n'


def bf_world(files=None, fakes=(), stubs=BF_STUBS, own_units=None, gametime=False, theme=True, whiteout_gametime=True):
    f = {"/boot/grub/grub.cfg": GRUB_CFG, "/etc/default/grub": DEFAULT_GRUB, "/etc/default/ufw": UFW_DEFAULT, "/etc/ufw/ufw.conf": "ENABLED=no\nLOGLEVEL=low\n",
         "/etc/fstab": "UUID=x / btrfs rw 0 0\n", "/usr/lib/systemd/user/.keep": "", "/usr/lib/systemd/system/.keep": ""}
    for u in (own_units or ()):
        f[u] = "[Unit]\n"
    if gametime:
        f["/usr/bin/yggdrasil-gametime"] = ("#!/bin/sh\n", 0o755)
    if theme:
        f["/usr/local/bin/labyrinth-grub-theme"] = (THEME_STUB.replace("@W@", W), 0o755)
    f.update(files or {})
    return world(files=f, stubs=stubs, fakes=list(fakes), whiteouts={"/usr/bin": ["yggdrasil-gametime"] if whiteout_gametime and not gametime else []})


def bf(name, steps=None, env=None, differs=(), **k):
    case(BF, name, steps or [{"args": []}], bf_world(**k), env=dict({"LABYRINTH_SWAP_GIB": "1"}, **(env or {})), differs=differs)


ALL_UNITS = ["/usr/lib/systemd/user/openrazer-daemon.service", "/usr/lib/systemd/user/yggdrasil-weather-record.timer",
             "/usr/lib/systemd/user/labyrinth-appnet-prepare.service", "/usr/lib/systemd/user/labyrinth-appnet-ask-agent.service",
             "/usr/lib/systemd/user/labyrinth-appusage.service", "/usr/lib/systemd/user/labyrinth-scan.timer",
             "/usr/lib/systemd/system/clamav-freshclam-once.timer", "/usr/lib/systemd/system/clamav-freshclam.service"]
bf("no grub.cfg", files={"/boot/grub/grub.cfg": ""})
bf("grub.cfg without menu entries", files={"/boot/grub/grub.cfg": "set timeout=5\nsubmenu 'x' {\n}\n"})
bf("grub.cfg without initrd", files={"/boot/grub/grub.cfg": "menuentry 'x' {\n linux /vmlinuz\n}\n"})
bf("grub.cfg a link to nothing", files={"/boot/grub/grub.cfg": ("link", "/nowhere")})
# grep's complaint about the missing grub.cfg.new is not repeated
bf("a plain world: not btrfs, nothing optional", stubs=("systemctl", "pacman", "findmnt", "grub-mkconfig"), theme=False, differs=(("err", 0),))
bf("everything: btrfs, all units, ufw, theme", fakes=BTRFS_ROOT + [("pacman_-Qq_libratbag", {}), ("pacman_-Qq_kdeconnect", {})],
   own_units=ALL_UNITS, gametime=True, files={"/etc/ufw/applications.d/kdeconnect": "[KDE Connect]\n"})
bf("libratbag and kdeconnect not installed, freshclam daemon", fakes=BTRFS_ROOT + [("pacman_-Qq", {"rc": 1})],
   own_units=["/usr/lib/systemd/system/clamav-freshclam.service"])
bf("kdeconnect already allowed", fakes=BTRFS_ROOT + [("pacman_-Qq_kdeconnect", {})],
   files={"/etc/ufw/applications.d/kdeconnect": "x\n", "/etc/ufw/user.rules": "### tuple ### allow KDE Connect\n"})
bf("every enable fails", fakes=BTRFS_ROOT + [("systemctl", {"rc": 1}), ("heimdall-enable-defaults", {"rc": 3, "out": "heimdall says no\n"}),
                                             ("labyrinth-hardware-setup", {"rc": 1})], own_units=ALL_UNITS, gametime=True)
bf("ufw's files missing", files={"/etc/default/ufw": None, "/etc/ufw/ufw.conf": None}, differs=(("out", 0), ("err", 0)))
bf("GRUB settings: odd lines, no newline at the end (fixed: a line of its own)", fakes=BTRFS_ROOT, files={
    "/etc/default/grub": "GRUB_TIMEOUT=10\n  GRUB_TIMEOUT=3\nGRUB_TIMEOUTX=1\nGRUB_CMDLINE_LINUX_DEFAULT='a'\nGRUB_CMDLINE_LINUX_DEFAULT=\"b c\"\n"
                         "GRUB_CMDLINE_LINUX_DEFAULT=open\nGRUB_DISABLE_OS_PROBER=true\n##GRUB_DISABLE_OS_PROBER=x\nGRUB_TOP_LEVEL=keep",
    "/etc/fstab": "UUID=x / btrfs rw 0 0"})
bf("GRUB settings: already there", fakes=BTRFS_ROOT, files={
    "/etc/default/grub": "GRUB_CMDLINE_LINUX_DEFAULT=\"usbcore_autosuspend=1\"\nGRUB_TOP_LEVEL=\"x\"\n"})
bf("GRUB settings: no /etc/default/grub", fakes=BTRFS_ROOT, files={"/etc/default/grub": None})
bf("hibernate: the size from memory, too big for the free space", fakes=BTRFS_ROOT, env={"LABYRINTH_SWAP_GIB": None})
for gib in ("0", "abc", "", "01", "99999999999999999999999"):
    bf("hibernate: LABYRINTH_SWAP_GIB=%r" % gib, fakes=BTRFS_ROOT, env={"LABYRINTH_SWAP_GIB": gib})
bf("hibernate: the subvolume cannot be made", fakes=BTRFS_ROOT + [("btrfs_subvolume_create", {"rc": 1})])
bf("hibernate: the swap file cannot be made", fakes=BTRFS_ROOT + [("btrfs_filesystem_mkswapfile", {"rc": 1})])
bf("hibernate: no offset", fakes=BTRFS_ROOT + [("btrfs_inspect-internal_map-swapfile", {"out": "x12\n"})])
bf("hibernate: a UUID that is no UUID (fixed: refused)", fakes=BTRFS_ROOT + [("findmnt_-no_UUID_%", {"out": "a|b\\1&\n"})])
bf("hibernate: no UUID", fakes=BTRFS_ROOT + [("findmnt_-no_UUID_%", {"out": ""})])
bf("hibernate: fstab has it already", fakes=BTRFS_ROOT, files={"/etc/fstab": "/swap/swapfile none swap 0 0\n"})
bf("snapper fails", fakes=BTRFS_ROOT + [("snapper_--no-dbus_-c_root_create-config", {"rc": 1, "err": "no\n"})])
bf("snapper's snapshot fails, no grub-btrfsd", fakes=BTRFS_ROOT + [("snapper_--no-dbus_-c_root_create", {"rc": 1})],
   stubs=tuple(s for s in BF_STUBS if s != "grub-btrfsd"))
bf("grub-mkconfig fails", fakes=BTRFS_ROOT + [("grub-mkconfig_-o", {"rc": 1})])
bf("grub-mkconfig writes no menu", fakes=BTRFS_ROOT + [("grub-mkconfig_-o", {"sh": 'echo "set x" > "$2"\n'})])
bf("the theme fails", fakes=BTRFS_ROOT, files={"/usr/local/bin/labyrinth-grub-theme": ("#!/bin/sh\necho broken >&2\nexit 1\n", 0o755)})
bf("the theme is not executable", fakes=BTRFS_ROOT, files={"/usr/local/bin/labyrinth-grub-theme": ("#!/bin/sh\n", 0o644)})
bf("zram: its file is a folder (fixed: a warning)", files={"/etc/systemd/zram-generator.conf/x": ""}, differs=(("err", 0),))

# ---------------------------------------------------------------- labyrinth-install-cleanup

IC = "labyrinth-install-cleanup"
PRESET = "ALL_kver=\"/boot/vmlinuz-%PKGBASE%\"\nPRESETS=('default')\ndefault_image=\"/boot/initramfs-%PKGBASE%.img\"\n"


def ic_tree(kernels=(("6.17.7-2-cachyos", "linux-cachyos"), ("6.12.57-1-cachyos-lts", "linux-cachyos-lts")), live=True, template=PRESET, extra=None):
    f = {"/usr/share/mkinitcpio/hook.preset": template, "/etc/mkinitcpio.d/linux-cachyos.preset": "archiso preset\n",
         "/etc/mkinitcpio.d/old.preset": "x\n", "/etc/mkinitcpio.d/keep.conf": "x\n", "/etc/mkinitcpio.d/.hidden.preset": "x\n"}
    for d, base in kernels:
        if base is not None:
            f["/usr/lib/modules/%s/pkgbase" % d] = base + "\n"
        f["/usr/lib/modules/%s/vmlinuz" % d] = "kernel " + d + "\n"
    if live:
        f.update({"/home/live/.config/app/settings.ini": ("x=1\n", 0o600), "/home/live/.config/run.sh": ("#!/bin/sh\n", 0o700),
                  "/home/live/.config/shared/": ("dir", 0o777), "/home/live/.config/link": ("link", "app/settings.ini"),
                  "/home/live/Desktop/install.desktop": "x\n", "/etc/greetd/live.toml": "x\n", "/etc/greetd/config.toml": "keep\n",
                  "/etc/systemd/system/greetd.service.d/live.conf": "x\n", "/etc/sudoers.d/00-live-nopasswd": "x\n",
                  "/etc/systemd/system/getty.target.wants/serial-getty@ttyS0.service": ("link", "/usr/lib/systemd/system/serial-getty@.service"),
                  "/usr/share/applications/labyrinth-install.desktop": "x\n", "/etc/mkinitcpio.conf.d/archiso.conf": "x\n"})
    for k in (extra or {}):
        for other in list(f):
            if other != k and k.startswith(other.rstrip("/") + "/"):
                del f[other]
    f.update(extra or {})
    return f


def ic_files_outside(f):
    """The same files for a run with LABYRINTH_ROOT (no chroot): all under WORLD/root, /usr included."""
    def go(paths):
        for path, v in f.items():
            p = R + path
            os.makedirs(os.path.dirname(p.rstrip("/")), exist_ok=True)
            if isinstance(v, tuple) and v[0] == "link":
                os.symlink(v[1], p)
            elif isinstance(v, tuple) and v[0] == "dir":
                os.makedirs(p, exist_ok=True)
                os.chmod(p, v[1])
            elif isinstance(v, tuple):
                P.write(p, v[0], v[1])
            elif v is not None:
                P.write(p, v)
        os.makedirs(os.path.join(W, "bin"), exist_ok=True)
        os.makedirs(os.path.join(W, "sysbin"), exist_ok=True)
        for n in SYSBIN:
            if os.path.exists("/usr/bin/" + n):
                os.symlink("/usr/bin/" + n, os.path.join(W, "sysbin", n))
        for n in ("getent", "userdel", "chattr", "findmnt"):
            P.write(os.path.join(W, "bin", n), FAKE.replace("@W@", W), 0o755)
    return go


def ic_outside(name, as_root=False, differs=(), **k):
    steps = [{"args": [], "wrap": ["/usr/bin/unshare", "-r", "--"] if as_root else []}]
    P.case(IC, name, steps, setup=ic_files_outside(ic_tree(**k)), env={"LABYRINTH_ROOT": R, "PATH": W + "/bin:" + W + "/sysbin", "LANG": "C.UTF-8"},
           differs=differs)


ic_outside("a mock root, as a normal user (tests/test_install_cleanup.sh's way)")
ic_outside("a mock root, as root of a namespace (skel's owner made root)", as_root=True)
ic_outside("no live user", live=False)
ic_outside("no template", template=None)
ic_outside("an empty template", template="")
ic_outside("a template without PRESETS", template="ALL_kver=x\n")
ic_outside("no kernels", kernels=())
ic_outside("a kernel without pkgbase is skipped, no LTS", kernels=(("6.17.7-2-cachyos", "linux-cachyos"), ("6.1-x", None)))
ic_outside("an odd pkgbase", kernels=(("6.17.7-2-cachyos", "linux cachyos|x"),))
ic_outside("a pkgbase with a NUL and trailing newlines", kernels=(("6.17.7-2-cachyos", "linux-cachyos\0\n\n"),), differs=(("err", 0),))
ic_outside("a live-only path that is a folder", extra={"/etc/sudoers.d/00-live-nopasswd/x": "x\n"}, differs=(("err", 0),))
ic_outside("serial-getty is a folder", extra={"/etc/systemd/system/serial-getty@ttyS0.service/x": "x\n"})
ic_outside("a preset that is a folder", extra={"/etc/mkinitcpio.d/dir.preset/x": "x\n"}, differs=(("err", 0),))
ic_outside("skel already there", extra={"/etc/skel/.config/app/old.ini": ("old\n", 0o600), "/etc/skel/": ("dir", 0o700), "/etc/skel/.bashrc": "x\n"})
ic_outside("kernels in another order", kernels=(("7.10.1-1", "linux-a"), ("7.9.0-1", "linux-b"), ("7.1.0-1", "linux-cachyos-lts")))


def ic_chroot(name, fakes=(), **k):
    f = ic_tree(**k)
    case(IC, name, [{"args": []}], world(files=f, stubs=("getent", "userdel", "chattr", "findmnt"), fakes=list(fakes), own=("/usr/lib/modules",)))


ic_chroot("the installed system: the live account removed, /boot NOCOW", fakes=[("findmnt_-no_FSTYPE_%", {"out": "btrfs\n"}), ("getent_passwd_live", {"out": "live:x:1000:1000::/home/live:/bin/bash\n"})])
ic_chroot("no live account, not btrfs", fakes=[("getent", {"rc": 2}), ("findmnt_-no_FSTYPE_%", {"out": "ext4\n"})])
ic_chroot("userdel and chattr fail", fakes=[("findmnt_-no_FSTYPE_%", {"out": "btrfs\n"}), ("userdel", {"rc": 8}), ("chattr", {"rc": 1, "err": "chattr: no\n"})])
