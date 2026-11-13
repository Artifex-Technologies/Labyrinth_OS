# labyrinth-publish-repo: the daily package publish, in Rust

Labyrinth's package repository (the release **packages** of github.com/Artifex-Technologies/Labyrinth_OS, the `[labyrinth]`
repository every installed system updates from) is filled by this program: fresh copies of Labyrinth_OS, Yggdrasil, Sarina,
Heimdall and Limbo from GitHub, the changed packages built (`scripts/build-local-packages.sh` and the store's recipes), each
package and the database signed with Labyrinth's key, the packages uploaded, the database last, and the files of the publish
before last removed. What it does and why is in [docs/PACKAGE_REPO.md](../../docs/PACKAGE_REPO.md); the versions it compares
in [docs/VERSIONING.md](../../docs/VERSIONING.md).

It is the Rust port of `scripts/publish-repo.py` (2026-11-13): the same options (`--force`, `--dry-run`), environment
(`LABYRINTH_PUBLISH_DIR`, `LABYRINTH_GNUPGHOME`), steps, calls (by argv, never a shell), files and messages. The Arch tools it
calls stay what they are: git, makepkg, bsdtar, gzip, vercmp, gpg, repo-add and gh. It is not a package: it runs only on the
PC that holds the signing key, from my user timer.

## Building and switching the timer to it

```
cd packages/publish-repo
cargo build --release --locked
install -Dm755 target/release/labyrinth-publish-repo ~/.local/bin/labyrinth-publish-repo
```

`scripts/systemd/labyrinth-publish.service` starts `%h/.local/bin/labyrinth-publish-repo`. The installed unit is switched
by copying it again (`cp scripts/systemd/labyrinth-publish.service ~/.config/systemd/user/ && systemctl --user daemon-reload`),
or by changing its `ExecStart=` line to that path. A test without uploading: `~/.local/bin/labyrinth-publish-repo --dry-run`.

The timer runs the installed copy, not the checkout: a change here reaches the daily publish when I build and install it
again (the script ran from my working tree, unfinished edits included).

`scripts/publish-repo.py` stays for a rollback (put the old `ExecStart=` back) until one week after the timer first runs this
program; then it goes, with `tests/test_publish_repo.py` and the parity harness's old side
([docs/RUST_PORT_REMAINING.md](../../docs/RUST_PORT_REMAINING.md), "Copies kept for a rollback").

## Where it is not 1:1

- When it stops (a program failed, a file it needs is missing or not what it should be) it says why in one line on standard
  error and exits with 1; the script printed a Python traceback (also exit code 1). The script's own messages (the missing
  key, a file name GitHub would rename) are word for word the same. A failed program's own error output, which the script
  swallowed where it read the output (bsdtar, vercmp, git), is in that line.
- `--help` is its own text; a wrong option is refused as argparse refused it (exit code 2, the same message).
- The release's notes, written only when the release is made, name this program instead of the script.

Everything else is the script's behaviour, including what is arguably a flaw: published.json without a list of files stops
the publish after the upload (the database is online, the state is not written, the next run publishes again).

## The differential (2026-11-13)

`parity/compare.py` runs the script and the program on the same made-up worlds and compares, step by step: exit code,
standard output, standard error (all but the last words when it stops), every call in order (program, arguments, folder and
the environment the publish sets), every file of the work folder and of the stand-in release (contents and modes; the
databases by the files inside them, and their signatures checked as good). It runs in a user, mount, PID and network
namespace: no network, the world on a tmpfs that is gone afterwards, my real signing keyring and gh's settings covered by
empty folders, a throw-away ed25519 key generated for the run. git, gh, makepkg and build-local-packages.sh are stand-ins
(`parity/standin.py`); gpg, bsdtar, gzip, vercmp and repo-add are the real ones, on real packages made for the run.

```
cargo build --release && python3 -I parity/compare.py [--only CASE] [-v]
```

22 cases, 33 publishes, all the same: a first publish (the release made, a stale signature beside a package replaced, an
epoch's colon renamed, a build left over without a recipe and the store's folder without one skipped), nothing changed,
`--force` with nothing new, new builds of the same program kept as published, a build from a home folder replaced, a release
published, a recipe taken out and its files removed one publish later; a changed build whose version is not newer (the
warning; the published one kept); `--dry-run` and `--dry`; the key missing, the keyring folder missing; the ISO's build and
a store build failing; fetch and clone failing; origin's HEAD not a remote branch; a partial upload (the second batch of 20
fails after one file, the next run uploads only what is missing); the database's upload failing; repo-add failing; a file
name GitHub would rename; a package without a pkgname; published.json not JSON, not an object, without files, with files as
a string; `--help`, an unknown option, `--force=yes`.

The differential found no bug in the script. It found three things about the made-up world that a real run depends on: the
real repo-add here (pacman 7.1's, in Rust) compresses the database through `sh -c "gzip -c -f -n"`, so `sh` and `gzip` must be
on the publish's PATH; a program started from Python inherits Python's ignored SIGPIPE through `exec` (bsdtar then reports a
"Write error" instead of ending quietly; both the script and this program start their tools with SIGPIPE restored); and the
database's archive carries the time it was made, so its signature differs from run to run while the database is the same.

## Why Rust: measured

`parity/bench.py`, 2026-11-13, my desktop (16 threads, Linux 7.3, Python 3.14.9, Rust 1.99.1), the world of the differential
after one real publish of 30 packages, the stand-ins swapped for small shell ones (gpg, bsdtar, gzip, vercmp and repo-add are
the real ones). A C launcher starts each run (wall time, CPU of the run with every tool it starts, the largest single process
from wait4, and the program's own peak memory read as it exits through ptrace). 30 runs of each for `--help` and the daily
"nothing changed", 7 for `--force` (all 30 packages read, compared, their signatures checked, the database made, signed and
uploaded); medians, p10-p90 in brackets.

| Case | Wall, script → Rust | CPU (with the tools) | The program's own peak memory |
| --- | --- | --- | --- |
| `--help` | 43.7 (43.2-44.5) → 0.67 (0.62-0.71) ms, −98 % | 43.4 → 0.42 ms | 20.9 → 2.6 MB |
| nothing changed (the usual 05:30 run) | 67.3 (66.7-67.9) → 22.2 (21.5-22.6) ms, −67 % | 66.0 → 21.0 ms | 20.9 → 2.8 MB |
| `--force`, 30 packages | 721 (718-721) → 642 (640-644) ms, −11 % | 820 → 750 ms | 21.2 → 2.9 MB (largest process of the run: 21.2 → 15.0 MB, a tool) |

What is left of the Rust run is the tools: in "nothing changed" mostly gpg checking that the key is there, in `--force` gpg,
bsdtar and repo-add, which both sides start the same number of times. The 45-80 ms the script spends of its own per run was
Python starting, its imports and its own work. Speed is not why: the timer runs once a day. Why: one less Python program running
unattended with the signing key's keyring at hand; no interpreter or module search path in that run; every program started
by argv with the publish's own environment; a failure says what failed in one line in the journal instead of a traceback.
On disk the binary is 0.53 MB against the script's 18 KB, which needed Python (75 MB, installed anyway on my PC).
