# Labyrinth's package repository

How an installed Labyrinth system stays current without a new ISO.

## What it is

The release named **packages** of github.com/Artifex-Technologies/Labyrinth_OS is a pacman repository: Labyrinth's own
software, each package and the database signed with the Labyrinth key. Every installed system lists it last in
`/etc/pacman.conf`:

```
[labyrinth]
SigLevel = Required
Server = https://github.com/Artifex-Technologies/Labyrinth_OS/releases/download/packages
```

So `pacman -Syu` and Agora's updates bring the desktop (Yggdrasil), Sarina, Heimdall, Limbo and the store's apps up to
date, from any ISO. Last on purpose: a package of the same name in Arch's or CachyOS's repositories always wins, so this
repository can add Labyrinth's software but never replace the base system.

| In the repository | Not in it, and why |
|---|---|
| Everything built from Yggdrasil (panel, Settings, compositor, Agora, Minos, spell check, Liki, ...), Sarina, Heimdall, Limbo, labyrinth-keyring, labyrinth-distress, labyrinth-ollama-tuning, labyrinth-framegen, Daedalus CAD, the store's apps (Huginn, Timer, 2048, Sudoku, Authenticator, Health, controllers, agent usage, browser warm-up) | **Proton Pass, Proton Mail, VS Code**: their licences do not let Labyrinth pass them on. Agora installs them from the AUR (`proton-pass-bin`, `proton-mail-bin`, `visual-studio-code-bin`), downloaded from Proton and Microsoft, and its updates keep them current (`aur_updates_ok` in the catalog). |

## The key

`C9CE784C8BD9B661042075972D3BD4B1E7CD07A5`, "Labyrinth OS packages", ed25519, signing only. Installed systems trust it
through the **labyrinth-keyring** package (on the ISO; `pacman-init.service` populates it on first boot).

- On the build PC: its own keyring, `~/.local/share/labyrinth-signing/gnupg` (mode 700), so no other key is ever used and
  the owner's own GnuPG is not touched. No passphrase, so the daily publish can run unattended.
- Backup: Proton Drive, `Documents/Github/Labyrinth_OS/package-signing-key/` (the secret key, the public key, the revocation
  certificate and how to restore). Never in a git repository.
- If it is ever stolen: publish a new labyrinth-keyring with a new key and the old one listed in `labyrinth-revoked`, sign
  that with the old key one last time, then revoke the old key.

## Versions

A package built from source here keeps the version its PKGBUILD names until someone raises it by hand, so
`build-local-packages.sh` stamps it with the commit count of the repository it comes from: `0.1.0-27` becomes
`0.1.0.r405-27`, which pacman orders after `0.1.0-27` and after every earlier stamp. `-git` recipes already do this
in `pkgver()`. A new build whose files and dependencies are the same as the published one is not published again.

## Publishing

`scripts/publish-repo.py` builds from fresh copies of each repository's `main` on GitHub (never a working tree), in
`~/.cache/labyrinth-publish`, then signs, uploads the packages, uploads the database last, and removes the files of the
publish before last. It does nothing when no repository changed (`--force` to publish anyway, `--dry-run` to build and sign
without uploading). Builds run with `makepkg -c`, so no build files are left behind, and the store's apps build in a fixed
folder, so an unchanged app is recognised as unchanged and not uploaded again. The source repositories are reset with
`git clean`; anything a recipe keeps next to itself (the clone of a `-git` recipe) must be in `.gitignore` or it is wiped.

Recipes that build from the publish copy itself (`git+file://`: heimdall-git, sarina-git, limbo-git) take its `main` branch, so
each copy's `main` is moved to GitHub's on every publish. Until 2026-11-07 (6238b19) the copies were checked out detached, `main`
stayed at the first clone, and the published Heimdall and Sarina were days behind.

On the build PC it can run daily at 05:30 (after Sarina's night training); switching the timer on is the owner's choice:

```
cp scripts/systemd/labyrinth-publish.{service,timer} ~/.config/systemd/user/
systemctl --user daemon-reload && systemctl --user enable --now labyrinth-publish.timer
journalctl --user -u labyrinth-publish     # what the last publish did
```

## The ISO build's own packages

The ISO takes Labyrinth's software from packages built on the build PC from the same recipes (`scripts/build-local-packages.sh`
puts them in `repo/x86_64`). Since
2026-11-07 they are signed too, and checked like any repository:

- `build-local-packages.sh` signs every package and the database with the Labyrinth key, from the same keyring as above
  (`LABYRINTH_GNUPGHOME` points elsewhere). Without the key it stops and says so.
- `scripts/build.sh` adds the key to the build host's pacman from this checkout's `labyrinth-keyring` package, checks its full
  fingerprint, and only then signs it locally.
- `profile/pacman.conf` has `SigLevel = Required` for the local `[labyrinth]` repository.
- `LABYRINTH_UNSIGNED=1` (for both scripts) builds once without signatures on a host that does not have the key; `build.sh`
  then trusts the local packages unsigned for that build, and says so.

## Agora

- Components with their own recipe (Limbo, Sarina, Heimdall, the store's apps): `labyrinth-apps` installs the
  repository's signed build when pacman has it (`pacman -S --needed`, which also updates an older one), and builds the
  recipe on the machine only when the repository cannot be reached.
- Agora shows an Update button on a component when any of its packages has an update (repository or AUR).
