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
| Everything built from Yggdrasil (panel, Settings, compositor, Agora, Minos, spell check, Liki, ...), Sarina, Heimdall, Limbo, labyrinth-keyring, labyrinth-distress, labyrinth-ollama-tuning, Kairos (frame generation), Daedalus CAD, Argus (LykanOS's: its Agora lists it, Labyrinth's does not; not on Labyrinth's ISO), the store's apps (Timer, 2048, Sudoku, Authenticator, Health, controllers, agent usage, browser warm-up) | **Proton Pass, Proton Mail, VS Code**: their licences do not let Labyrinth pass them on. Agora installs them from the AUR (`proton-pass-bin`, `proton-mail-bin`, `visual-studio-code-bin`), downloaded from Proton and Microsoft, and its updates keep them current (`aur_updates_ok` in the catalog). |

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

Since 2026-11-11 every package has a version and a build number: the system version (Labyrinth OS 1.0) for Labyrinth's and
Yggdrasil's own, each app's own for Sarina, Heimdall, Limbo, Kairos, Daedalus and Argus, and as `pkgrel` the count of commits
since that release which touch the package's source (`1.0.0-37`). Each recipe works them out itself; the whole scheme, and how to
make a release, is in [VERSIONING.md](VERSIONING.md). (Before, `build-local-packages.sh` stamped the repository's commit count
into the version: `0.1.0.r405-27`.) A new build whose files and dependencies are the same as the published one, at the same
version, is not published again; a release always is.

The store's Yggdrasil apps (`yggdrasil-2048-git` and the other plugins; `huginn-git` until 2026-11-08, when Huginn moved to a private repository) have `epoch=1` since 2026-11-07, and `argus-git` since 2026-11-11 (makepkg then puts a colon in the file name, `name-1:ver`, which GitHub does not keep: the publish names those files with a dot instead, `name-1.ver`, and the database lists that name; 2026-11-08). The apps' version
ended in Yggdrasil's newest commit, so two builds with the same count were ordered at random; Argus's were commit counts
(25.3078) that 1.0.0 sorts before without the epoch.

## Build settings

Everything here is built for any 64-bit PC: both build scripts give makepkg `scripts/makepkg.conf`, which takes the build
machine's settings and replaces only the compiler's CPU target (`-march=x86-64`, Rust's default CPU). A plain `makepkg` on
CachyOS builds for the machine's own CPU (`-march=native`, `target-cpu=native`), and the program then stops with "illegal
instruction" on other processors. Until 2026-11-07 every Rust program in the repository was built that way, for an i9-9900K;
the rebuilds went out as updates (new commit counts, `pkgrel` 2 for daedalus-cad-git and labyrinth-framegen-git). Go programs
were never affected (`CGO_ENABLED=0`, Go's own baseline CPU).

No account name in the packages (2026-11-07): a program records the source paths it was built from (Rust and C++ panic and
assertion messages), and each package's `.BUILDINFO` records the folder it was built in. Both were under the builder's home
folder, so every published package named the account. `scripts/makepkg.conf` now writes the home folder as `/build` inside
programs (`--remap-path-prefix`, `-ffile-prefix-map`), and the publish builds in `/var/lib/labyrinth-publish` instead of
`~/.cache/labyrinth-publish`. Every recipe's `pkgrel` went up by one so the clean builds replace the old files online; the
publish keeps an unchanged program's published file only when that file was not built under `/home` or `/root`.

## Publishing

`scripts/publish-repo.py` builds from fresh copies of each repository's `main` on GitHub (never a working tree), in
`/var/lib/labyrinth-publish` (outside the home folder, see above; made once with
`sudo install -d -o $USER -g $USER /var/lib/labyrinth-publish`, or set `LABYRINTH_PUBLISH_DIR`), then signs, uploads the packages, uploads the database last, and removes the files of the
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
