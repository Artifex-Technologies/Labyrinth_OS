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

Every package in it is installed by something: the ISO's list, a component (what the installer and Agora install), Agora's
catalog, or another package's `depends` or `optdepends`. `tests/test_installed_by.py` (run by `scripts/release-check.sh`)
fails on a recipe nothing installs, here or in Yggdrasil; the few installed another way (Argus by LykanOS's Agora, `yara-x`
in Limbo's analysis image, the own compositor while it is tried by hand) are listed in it with the reason.

| In the repository | Not in it, and why |
|---|---|
| Everything built from Yggdrasil (panel, Settings, compositor, Agora, Minos, spell check, Liki, ...), Sarina, Heimdall, Limbo, labyrinth-keyring, labyrinth-distress, labyrinth-coreutils-rs and labyrinth-sudo-rs (they replace coreutils and sudo with their Rust versions: [RUST_BASE.md](RUST_BASE.md)), labyrinth-ollama-tuning, Kairos (frame generation), Daedalus CAD, Icarus, Argus (LykanOS's: its Agora lists it, Labyrinth's does not; not on Labyrinth's ISO), the store's apps (Timer, 2048, Sudoku, Authenticator, Health, controllers, agent usage, browser warm-up), **yara-x** (YARA-X's `yr`, built from VirusTotal's source: see below) and the reading fonts (Atkinson Hyperlegible Next and Mono, Lexend, OpenDyslexic and OpenDyslexic Mono: see below) | **Proton Pass, Proton Mail, VS Code**: their licences do not let Labyrinth pass them on. Agora installs them from the AUR (`proton-pass-bin`, `proton-mail-bin`, `visual-studio-code-bin`), downloaded from Proton and Microsoft, and its updates keep them current (`aur_updates_ok` in the catalog). |

## Built from someone else's source: yara-x

Arch's official repositories do not have YARA-X's command-line scanner `yr` (only the AUR does), and Limbo's analysis guest
image needs it (it replaced C YARA there on 2026-11-18; Sarina's `packaging/limbo/decompile/README.md` has how the image gets
it). So `packages/pkgbuilds/yara-x` builds it here, my own recipe (the AUR's was read for ideas only):

- VirusTotal's release tarball at the tag (`v1.23.0`), its SHA-256 pinned in the recipe; `cargo build --release --locked`
  with the release's own `Cargo.lock`, only the `yr` program. BSD-3-Clause, the licence installed with it.
- Its `check()` scans a one-rule file with the program just built (one file must match, one must not), not the library's
  whole test suite (minutes more for little).
- A clean build takes about 3 minutes and 1.4 GB of build files; the package is 8.5 MB (`yr` 30 MB, glibc and gcc-libs only).
- Pinned like any recipe not built from our own repositories: built once per version and skipped while that version is in
  the repository. Not on the ISO (nothing there needs it), so an ISO build leaves it out unless named.
- A new YARA-X release is my choice: `pkgver` and the checksum in the recipe, `pkgrel` back to 1. If Arch ever packages
  `yara-x`, theirs wins (this repository is listed last) and the recipe can go.

## Built from someone else's source: the reading fonts

The fonts to choose from in the installer and in Settings > Appearance > Font (Yggdrasil's `docs/APPS_SETTINGS.md`, decided
2026-11-18): Arch has only the original Atkinson Hyperlegible (`ttf-atkinson-hyperlegible`, taken from Arch), and OpenDyslexic
only as a 41 MB Nerd Font. So five small recipes here, my own, pinned like yara-x (the upstream version as `pkgver`, `pkgrel`
counting my rebuilds, every source's SHA-256 in the recipe, files from a repository at a fixed commit, never a branch:
`tests/test_package_version.py` checks all of it):

| Package | From | Licence | Package size |
| --- | --- | --- | --- |
| `ttf-atkinson-hyperlegible-next` 2.001 | Google Fonts (`google/fonts`, `ofl/atkinsonhyperlegiblenext`): one variable file and its italic | SIL OFL 1.1 | 128 KB |
| `ttf-atkinson-hyperlegible-mono` 2.001 | Google Fonts (`ofl/atkinsonhyperlegiblemono`), the same | SIL OFL 1.1 | 75 KB |
| `ttf-lexend` 1.007 | Google Fonts (`ofl/lexend`): one variable file | SIL OFL 1.1 | 103 KB |
| `otf-opendyslexic` 0.91.12 | the upstream release zip (regular, bold, italic, bold italic; not its web fonts) | SIL OFL 1.1 | 484 KB |
| `otf-opendyslexic-mono` 1.001 | the 2016 release of the older repository (`antijingoist/open-dyslexic`, tag 20160623-Stable): the only monospaced OpenDyslexic | Bitstream Vera (redistribution allowed, not to be sold on its own) | 52 KB |

All six are on the ISO (about 1.5 MB installed together), and Yggdrasil's packages depend on them, so a system installed
before they were added gets them with its next update: `yggdrasil-settings` on all six (Appearance > Fonts),
`yggdrasil-theme` on Atkinson Hyperlegible and Atkinson Hyperlegible Mono, the default system font since 2026-11-19. Each recipe's `check()` makes sure the files are the family they claim
to be (`fc-scan`). OpenDyslexic's sources moved from GitHub to forge.hackers.town in 2025; the release files are the same. A new
version is my choice: the commit or release, `pkgver` and the checksums, `pkgrel` back to 1. If Arch packages one of them, its
package wins (this repository is listed last).

Each of the five has a small scriptlet (`<name>.install`, since pkgrel 2 on 2026-11-18): after installing or updating, when
fontconfig does not list its family (`fc-list -q`), it makes the system's font caches again (`fc-cache -fs /usr/share/fonts`,
about two seconds); otherwise it runs nothing. The reason, found on my PC: `pacman -S ttf-atkinson-hyperlegible` and then, in
the same second, `pacman -U` of these five left `fc-list` showing only Atkinson Hyperlegible ("/usr/share/fonts/TTF: skipping,
existing cache is valid: 4 fonts"). Fontconfig keeps a folder's cache while the folder's modification time is the same, and
Arch's fontconfig 2.18.4 records that time in whole seconds (the cache's nanoseconds field is 0), so fontconfig's own hook
after the second transaction kept the first one's caches of /usr/share/fonts/TTF and of /usr/share/fonts (which then did not
list the new OTF folder at all). The files' own times and install paths play no part. Settings > Appearance > Fonts also
checks for this on its own (Yggdrasil's `docs/APPS_SETTINGS.md`, "a stale font cache"). `tests/test_package_version.py`
runs the scriptlets with stand-ins for fc-list and fc-cache.

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
Yggdrasil's own, each app's own for Sarina, Heimdall, Limbo, Kairos, Daedalus, Icarus and Argus, and as `pkgrel` the count of
commits since that release which touch the package's source (`1.0.0-37`). Each recipe works them out itself; the whole scheme,
and how to make a release, is in [VERSIONING.md](VERSIONING.md). (Before, the build script of the time stamped the
repository's commit count into the version: `0.1.0.r405-27`.) A new build whose files and dependencies are the same as the published one, at
the same version, is not published again; a release always is.

The store's Yggdrasil apps (`yggdrasil-2048-git` and the other plugins; `huginn-git` until 2026-11-08, when Huginn, the video app, moved to a private repository) have `epoch=1` since 2026-11-07, and `argus-git` since 2026-11-11 (makepkg then puts a colon in the file name, `name-1:ver`, which GitHub does not keep: the publish names those files with a dot instead, `name-1.ver`, and the database lists that name; 2026-11-08). The apps' version
ended in Yggdrasil's newest commit, so two builds with the same count were ordered at random; Argus's were commit counts
(25.3078) that 1.0.0 sorts before without the epoch.

## Build settings

Everything here is built for any 64-bit PC: every build gives makepkg `scripts/makepkg.conf`, which takes the build
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

`labyrinth-publish-repo` (the Rust program in `packages/publish-repo`; until 2026-11-13 `scripts/publish-repo.py`, kept for a
rollback until 2026-11-15, after the timer had run the program twice; git has it) builds from fresh copies of each repository's `main` on GitHub (never a working tree), in
`/var/lib/labyrinth-publish` (outside the home folder, see above; made once with
`sudo install -d -o $USER -g $USER /var/lib/labyrinth-publish`, or set `LABYRINTH_PUBLISH_DIR`), then signs, uploads the packages, uploads the database last, and removes the files of the
publish before last. It uploads nothing when no repository changed and nothing was built (`--force` to build everything and
publish anyway, `--dry-run` to build and sign without uploading). Builds run with `makepkg -c`, so no build files are left
behind, and the store's apps build in a fixed folder, so an unchanged app is recognised as unchanged and not uploaded again.
The source repositories are reset with `git clean`; anything a recipe keeps next to itself (the clone of a `-git` recipe)
must be in `.gitignore` or it is wiped.

### The builds (since 2026-11-21)

The publish and the ISO's local build (below) build the same way (`packages/publish-repo/src/build.rs`; until 2026-11-21
`scripts/build-local-packages.sh`, which built one recipe after another and stopped at the first failure):

- **Only what changed.** Each recipe has a key: the files makepkg says it makes (version and build number), the recipe as
  committed, the commit each of its git sources points at now (a sibling copy's `main`, or GitHub's for the store's apps),
  `scripts/makepkg.conf` (its settings, not its comments), and the checkout's uncommitted changes (none in the publish's fresh copies; in a working tree, any
  change rebuilds that checkout's recipes, since which recipe it feeds cannot be told). A recipe whose key matches its last
  good build, with that build's files still in `repo/x86_64`, is not built again. The keys are in `repo/builds.json`.
  The builds run even when none of the five repositories changed: the store's apps come from other repositories.
- **A few at a time.** Three builds at once, each with 4 compile jobs and at most 8 GB (its own `systemd-run --user --scope`,
  no swap), the longest first. `LABYRINTH_BUILDS_AT_ONCE`, `LABYRINTH_BUILD_JOBS` and `LABYRINTH_BUILD_MEMORY` (empty: no cap)
  change them.
- **A failure is named, and the rest go on.** The journal says which package failed, what makepkg was doing ("A failure
  occurred in check()."), and the lines of its log that say why (the failing tests and their assertions, the compiler's or
  linker's errors, a missing dependency, a kill for memory); each build's whole log is `repo/logs/NAME.log`. The other
  packages are published; the failed one keeps its last good build online and is built again on the next run. A failure
  that is the network's (a connection GitHub reset, a download that failed, a transfer stalled under 1 KB/s for a minute)
  is built once more a minute later first. The program then ends with exit code 1, so the unit shows as failed. (From 2026-11-18 to 2026-11-21 one failing package a day, a
  different one each day, held back every other change while the journal said only "exit status 4".)
- **makepkg installs nothing** (no `-s`): a missing build dependency is a failure that names it.
- **Each repository is downloaded once.** The store's apps name Yggdrasil as
  `${YGGDRASIL_SRC:-https://github.com/Artifex-Technologies/Yggdrasil.git}`; a user's system clones it from GitHub, the
  publish sets the variable to its own fresh copy (`file:///var/lib/labyrinth-publish/src/Yggdrasil`), the commit it
  builds everything else of Yggdrasil from. Any recipe source written that way for one of the five repositories is built
  from the copy. (On 2026-11-22 the line was slow and all eight store apps failed to clone Yggdrasil from GitHub.)

Recipes that build from the publish copy itself (`git+file://`: heimdall-git, heimdall-private-mic-git, heimdall-multilink-git, heimdall-yara-check-git, sarina-git, limbo-git) take its `main` branch, so
each copy's `main` is moved to GitHub's on every publish. Until 2026-11-07 (6238b19) the copies were checked out detached, `main`
stayed at the first clone, and the published Heimdall and Sarina were days behind.

On the build PC it can run daily at 05:30 (after Sarina's night training); switching the timer on is the owner's choice. The
timer runs the installed program, `~/.local/bin/labyrinth-publish-repo`, so it is built and installed first, and again after
every change to it:

```
(cd packages/publish-repo && cargo build --release --locked && install -Dm755 target/release/labyrinth-publish-repo ~/.local/bin/)
cp scripts/systemd/labyrinth-publish.{service,timer} ~/.config/systemd/user/
systemctl --user daemon-reload && systemctl --user enable --now labyrinth-publish.timer
journalctl --user -u labyrinth-publish     # what the last publish did
```

## The ISO build's own packages

The ISO takes Labyrinth's software from packages built on the build PC from the same recipes: `labyrinth-publish-repo build`,
run as a normal user inside this checkout, builds its recipes and those of the Yggdrasil checkout next to it (`../Yggdrasil`,
or `YGGDRASIL_DIR`) into `repo/x86_64`, the way the publish builds (above). Without names it builds what changed and leaves
out the apps that are not on the ISO (Daedalus CAD, Icarus, Argus, yara-x; `LABYRINTH_SKIP` replaces the list); named
recipes are always built (`labyrinth-publish-repo build yggdrasil-panel`), `--force` builds everything. Not installed? From
the checkout: `cargo run --release --manifest-path packages/publish-repo/Cargo.toml -- build`. Since 2026-11-07 the packages
are signed too, and checked like any repository:

- The build signs every package and the database with the Labyrinth key, from the same keyring as above
  (`LABYRINTH_GNUPGHOME` points elsewhere). Without the key it stops and says so.
- `scripts/build.sh` adds the key to the build host's pacman from this checkout's `labyrinth-keyring` package, checks its full
  fingerprint, and only then signs it locally.
- `profile/pacman.conf` has `SigLevel = Required` for the local `[labyrinth]` repository.
- `LABYRINTH_UNSIGNED=1` (for the build and for `build.sh`) builds once without signatures on a host that does not have the key; `build.sh`
  then trusts the local packages unsigned for that build, and says so.

## Agora

- Components with their own recipe (Limbo, Sarina, Heimdall, the store's apps): `labyrinth-apps` installs the
  repository's signed build when pacman has it (`pacman -S --needed`, which also updates an older one), and builds the
  recipe on the machine only when the repository cannot be reached.
- Agora shows an Update button on a component when any of its packages has an update (repository or AUR).
