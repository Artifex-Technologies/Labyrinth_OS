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
| Everything built from Yggdrasil (panel, Settings, compositor, Agora, Minos, spell check, Liki, ...), Sarina, Heimdall, Limbo, labyrinth-keyring, labyrinth-distress, labyrinth-ollama-tuning, the store's apps (Huginn, Timer, 2048, Sudoku, Authenticator, Health, controllers, agent usage, browser warm-up) | **Proton Pass, Proton Mail, VS Code**: their licences do not let Labyrinth pass them on. Agora installs them from the AUR (`proton-pass-bin`, `proton-mail-bin`, `visual-studio-code-bin`), downloaded from Proton and Microsoft, and its updates keep them current (`aur_updates_ok` in the catalog). **Daedalus CAD**: its repository is private. **Calamares**: being replaced by yggdrasil-installer. |

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
without uploading).

On the build PC it runs daily at 05:30 (after Sarina's night training):

```
cp scripts/systemd/labyrinth-publish.{service,timer} ~/.config/systemd/user/
systemctl --user daemon-reload && systemctl --user enable --now labyrinth-publish.timer
journalctl --user -u labyrinth-publish     # what the last publish did
```

## Agora

- Components with their own recipe (Limbo, Sarina, Heimdall, the store's apps): `labyrinth-apps` installs the
  repository's signed build when pacman has it (`pacman -S --needed`, which also updates an older one), and builds the
  recipe on the machine only when the repository cannot be reached.
- Agora shows an Update button on a component when any of its packages has an update (repository or AUR).
