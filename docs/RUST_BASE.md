# Rust in the base system: coreutils and sudo

Labyrinth's own programs are Rust. Two base packages are swapped for Rust ones too, because ready-made, packaged Rust
versions exist for them: GNU coreutils (`ls`, `cp`, `mv`, `rm`, `sort`, `head`, `wc` and the rest) and sudo. Nothing
here is my own rewrite: the programs come from Arch's `uutils-coreutils` and `sudo-rs` packages. The `[labyrinth]`
repository adds two small packages that put them under the usual names, and these are the only base packages it
replaces (CLAUDE.md, "The base system is not ours").

| Package | Replaces | What it holds |
| --- | --- | --- |
| `labyrinth-coreutils-rs` | `coreutils` | links in `/usr/bin`, one for each of GNU coreutils 9.12's 102 programs, to uutils' single program (`uu-coreutils`, which picks the tool by the name it was started as) |
| `labyrinth-sudo-rs` | `sudo` | `sudo`, `sudoedit` and `visudo` as links to `sudo-rs`, `sudoedit-rs` and `visudo-rs`; `/etc/sudoers`, `/etc/pam.d/sudo` and `/etc/sudoers.d` |

Both declare `replaces`, so `pacman -Syu` on any system that takes Labyrinth's packages offers the swap and makes it, also
with `--noconfirm`.

## Why

Both are memory-safe rewrites of programs that read untrusted input. sudo matters most: it runs as root and parses what
any user gives it, so a memory bug in it is a way to root; a memory bug in `ls` or `sort` is not one by itself. GNU's
programs do get such bugs now and then (a `sort` buffer over-read in 2025, for one).

What the swap does not buy: the Rust versions have their own bugs, of behaviour rather than memory. Ubuntu, which made
both its default in 25.10, found a `date` difference that broke its automatic updates. On Arch the coreutils run under
pacman's hooks, mkinitcpio and makepkg, so the swap ran on my PC before it reached any other machine.

## The sudo swap and the files sudo-rs needs

GNU sudo's package owns `/etc/sudoers` and `/etc/pam.d/sudo`, and sudo-rs's package ships neither. A plain swap (CachyOS's
`sudo-rs`, which conflicts with `sudo`) removes them: the PAM file goes, PAM falls back to its `other` service, which
denies everything, and no one can use sudo again without a live USB. So `labyrinth-sudo-rs` owns both files. During the
swap pacman removes sudo first and moves an edited `/etc/sudoers` (or PAM file) aside as `.pacsave`, then installs the
package's defaults; the install script puts the system's own files back (the defaults go to `.pacnew`), taking only a
`.pacsave` made in that transaction. It then checks the sudoers with `visudo-rs -c`. If sudo-rs cannot read it (an option
it does not support), it writes `/etc/sudoers-rs`, which sudo-rs reads instead, so the wheel group keeps sudo, and says
so; the rules in `/etc/sudoers` and `/etc/sudoers.d` then are not in use until the file is fixed.

## How it was checked (2026-11-19)

- **Every program is there.** All 102 names of GNU coreutils 9.12 (`pacman -Ql coreutils`), and nothing else, are in the
  package; uutils 0.12.0's program answered `--version` as uutils under each of them.
- **The packages are the real ones.** Arch's `uutils-coreutils` and `sudo-rs` (and CachyOS's swap packages, read for
  comparison) checked against pacman's keyring before anything ran.
- **The swap, in a throwaway Arch container**: GNU coreutils and sudo installed, a user in wheel with a password, the
  wheel line enabled in `/etc/sudoers`, and a no-password rule in `/etc/sudoers.d`. `pacman -Syu --noconfirm` with a test
  repository holding the two packages offered both replacements and made them. After it: `ls` and `sort` are uutils
  0.12.0; the edited `/etc/sudoers` is byte for byte the one from before (the default as `.pacnew`), and
  `visudo -c` reads it, including GNU's own default lines; sudo with the user's password, `sudo -i`, and the
  no-password rule work; `sort`, `date -d` and `-r`, `touch -d`, `stdbuf`, `wc`, `seq`, `tac`, `env -i`, `cp -a`, `mv`, `rm`,
  `mkdir -p`, `timeout`, `stat` and `test` give GNU's answers.
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

- `su` stays util-linux's: it is part of that package, which is not swapped. sudo-rs's `su-rs` is installed beside it.
- The ISO still installs GNU coreutils and sudo; an installed system swaps them at its first update.
