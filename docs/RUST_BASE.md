# Rust in the base system: coreutils and sudo

Labyrinth's own programs are Rust. Two base packages are swapped for Rust ones too, on Labyrinth and LykanOS alike:
GNU coreutils (`ls`, `cp`, `mv`, `rm`, `sort`, `head`, `wc` and the rest) and sudo. Neither is my own rewrite: coreutils
comes from uutils (Arch's `uutils-coreutils` package), sudo from sudo-rs (built here from its release). These are the only
base packages the `[labyrinth]` repository replaces (CLAUDE.md, "The base system is not ours"). Nothing in either comes
from CachyOS, so LykanOS, which takes no CachyOS packages, gets the same swap.

| Package | Replaces | What it is |
| --- | --- | --- |
| `labyrinth-coreutils-rs` | `coreutils` | links in `/usr/bin`, one for each of GNU coreutils 9.12's 102 programs, to uutils' single program (`uu-coreutils` from Arch's `uutils-coreutils`, which picks the tool by the name it was started as) |
| `labyrinth-sudo-rs` | `sudo`, `sudo-rs` | sudo-rs 0.2.15 built from the release tag's source (`sudo`, setuid root, `sudoedit`, `visudo`), with `/etc/sudoers`, `/etc/pam.d/sudo`, `/etc/sudoers.d` and the time-stamp folder (`/run/sudo/ts`) |

Both declare `replaces`, so `pacman -Syu` on any system that takes Labyrinth's packages offers the swap and makes it, also
with `--noconfirm`.

## Why

Both are memory-safe rewrites of programs that read untrusted input. sudo matters most: it runs as root and parses what
any user gives it, so a memory bug in it is a way to root; a memory bug in `ls` or `sort` is not one by itself. GNU's
programs do get such bugs now and then (a `sort` buffer over-read in 2025, for one).

What the swap does not buy: the Rust versions have their own bugs, of behaviour rather than memory. Ubuntu, which made
both its default in 25.10, found a `date` difference that broke its automatic updates. On Arch the coreutils run under
pacman's hooks, mkinitcpio and makepkg, so the swap runs on my PC for a week before it reaches other machines.

## Why sudo-rs is built here

Arch's `sudo-rs` installs beside GNU sudo, as `sudo-rs` and `visudo-rs`, and replaces nothing. CachyOS's replaces sudo
and ships its own `/etc/sudoers` and `/etc/pam.d/sudo`, but it is CachyOS's, and LykanOS takes nothing from CachyOS; a
package that relied on it made `pacman -Syu` fail outright on a system without CachyOS's repositories (I tried it). So
`labyrinth-sudo-rs` builds sudo-rs itself, pinned like `yara-x`: the release tarball with its checksum, the release's own
`Cargo.lock`. The `v0.2.15` tag is signed by an upstream maintainer (GitHub showed it verified on 2026-11-19). It replaces
GNU sudo and either distribution's sudo-rs.

The cost: sudo-rs's security releases reach installed systems only when the recipe is moved to them, not through Arch or
CachyOS. A new upstream release is published the same day it is noticed.

## The sudoers in the swap

During the swap pacman removes the old sudo first, moves an edited `/etc/sudoers` (or `/etc/pam.d/sudo`) aside as
`.pacsave`, and installs the package's defaults. The install script puts the system's own files back (the defaults go to
`.pacnew`), taking only a `.pacsave` made in that transaction. It then checks the sudoers with sudo-rs's `visudo -c`. If
sudo-rs cannot read it (an option it does not support), it writes `/etc/sudoers-rs`, which sudo-rs reads instead, so the
wheel group keeps sudo, and says so; the rules in `/etc/sudoers` and `/etc/sudoers.d` then are not in use until the file
is fixed.

sudo-rs does not support every option. The ones Labyrinth used and dropped: the agent account's `requiretty` and its
input/output recording (`log_input`, `log_output`, `iolog_dir`: sudo-rs has no recording; every command is still in the
journal with the user, the command and the folder), and `timestamp_type=global` (sudo-rs keeps one time stamp per
terminal). [AGENT_ACCOUNT.md](AGENT_ACCOUNT.md) has what that means for the agent.

## How it was checked (2026-11-19)

- **Every coreutils program is there.** All 102 names of GNU coreutils 9.12 (`pacman -Ql coreutils`), and nothing else,
  are in the package; uutils 0.12.0's program answered `--version` as uutils under each of them.
- **The packages are the real ones.** Arch's `uutils-coreutils` checked against pacman's keyring; sudo-rs's source from
  the signed release tag, its checksum in the recipe.
- **The swap, in throwaway Arch containers**, each with GNU coreutils and sudo installed, a user in wheel with a password,
  the wheel line enabled in `/etc/sudoers`, and a no-password rule in `/etc/sudoers.d`; `pacman -Syu --noconfirm` with a
  test repository holding the two packages, and no CachyOS repository:
  - **GNU sudo**: both replaced; `ls` and `sort` are uutils 0.12.0; the edited `/etc/sudoers` is byte for byte the one
    from before (the default as `.pacnew`); sudo with the user's password, `sudo -i` and the no-password rule work;
    `sort`, `date -d` and `-r`, `touch -d`, `stdbuf`, `wc`, `seq`, `tac`, `env -i`, `cp -a`, `mv`, `rm`, `mkdir -p`,
    `timeout`, `stat` and `test` give GNU's answers.
  - **A sudoers sudo-rs cannot read** (a `requiretty` line added): the install script says so and writes
    `/etc/sudoers-rs`; the wheel user's sudo with a password works, the no-password rule is (as it says) not in use.
  - **CachyOS's sudo-rs installed before** (as on my PC): `pacman -Syu` replaces it with `labyrinth-sudo-rs`; the sudoers
    kept, sudo and the no-password rule work.
- The first container run, before the install script put the sudoers back, showed why it must: the wheel user lost sudo.

## Going back

`pacman -S coreutils` takes coreutils back in one step. For sudo, the same `.pacsave` move happens in reverse, and GNU
sudo puts nothing back, so run both steps in one root command:

```bash
sudo sh -c 'pacman -S sudo && if [ -e /etc/sudoers.pacsave ]; then mv -f /etc/sudoers.pacsave /etc/sudoers; fi'
```

If sudo itself no longer works, `run0` (systemd's, through polkit, not sudo) runs the same command as root for a member
of wheel, and pacman's snapshots (snap-pac) are in the boot menu.

## Known limitations

- `su` stays util-linux's: it is part of that package, which is not swapped.
- The ISOs still install GNU coreutils and sudo; an installed system swaps them at its first update.
