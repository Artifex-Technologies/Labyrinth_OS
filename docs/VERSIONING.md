# Versions

Decided 2026-11-11. Labyrinth OS is numbered the way an iPhone is: the system has one version (Labyrinth OS 1.0, 1.1, 1.1.1),
and the apps that stand on their own have their own, like apps from the App Store.

| What | Its version | Where the number lives |
| --- | --- | --- |
| Labyrinth OS, the system | the system version | `VERSION` in this repository |
| Labyrinth's own packages (labyrinth-tools, labyrinth-distress, labyrinth-ollama-tuning) | the system version | the same file |
| Yggdrasil, the desktop: every package in its `packages/pkgbuilds`, and its apps in the store (2048, Sudoku, Timer, ...) | the system version, like Apple's built-in apps | `VERSION` in Yggdrasil, always the same as this repository's |
| Heimdall, Limbo, Sarina, Kairos, Daedalus CAD, the Argus app, Huginn, Icarus | each its own | `VERSION` in each one's repository |
| The Argus firmware | its own `ARG-<upstream>r<N>` names, as before | its git tags |
| labyrinth-keyring, yggdrasil-spell-packs | the date of the data they carry (the keys, the dictionaries' snapshot), as Arch's keyring does | their PKGBUILDs |

## Before 1.0

Labyrinth began as a CachyOS + KDE Plasma setup. Yggdrasil, our own desktop on Hyprland with our own installer, replaced KDE in
October and November 2026, and the tools moved to Rust. None of those builds carried a release number (the packages were
`0.1.0.r<commit count>`), so 1.0 is the first numbered release.

## The numbers

`MAJOR.MINOR.PATCH`, and they move only when I make a release:

- **PATCH** (1.0.0 to 1.0.1): a fix release.
- **MINOR** (1.0.1 to 1.1.0): a feature release.
- **MAJOR** (1.1.0 to 2.0.0): a big release.

Everything in between raises only the **build number**, which is pacman's `pkgrel`: `1.0.0-37` is the 37th build of 1.0.0. So the
daily publish still reaches every installed system, and pacman always sees the newer build as an update. A release starts the
build number again at 1 (`1.1.0-1`).

Everything started at 1.0.0 on 2026-11-11. Each package's new version sorts after the one it replaces (`vercmp 1.0.0-1
0.1.0.r536-38` is 1); `argus-git` needed `epoch=1` for that (its old versions were commit counts like 25.3078), and the store's
Yggdrasil apps and huginn-git keep the epoch they already had.

## Where it shows

- **Settings > About**: "Labyrinth OS 1.0", from `/etc/os-release`.
- **`/etc/os-release`**: `VERSION="1.0"` (the patch only when there is one: "1.1", "1.1.1") and `VERSION_ID=1.0.0`; ID and the
  rest stay. The medium has them from `profile/airootfs/etc/os-release`; an installed system gets them from **labyrinth-tools**,
  whose install script writes them on every update (only where `ID=labyrinth`; a system that links its os-release to the
  distribution's is left alone). So an update to 1.1 shows 1.1.
- **The ISO's name**: `labyrinth-1.0.0-2026.11.11-x86_64.iso` (version, then the day it was built); `scripts/build.sh` stops if
  os-release does not say the same version as `VERSION`.
- **pacman**: `pacman -Q yggdrasil-settings` says, for example, `1.0.0-12`.

## How a package gets its numbers

Every recipe works its version out itself, so a plain `makepkg` in its folder gives the same numbers as
`scripts/build-local-packages.sh` and the publish. The helper is `scripts/package-version.sh` (`_version CHECKOUT [PATH...]`):

- **the version** is the `VERSION` file in the source's repository, as committed;
- **the release** is the commit that last changed that file (the one `scripts/release.sh` makes and tags `vX.Y.Z`);
- **the build number** is 1 + the commits since the release that touch what the package is built from: the folders its recipe
  names, the Rust crates those use from the same repository (their path dependencies, found in each `Cargo.toml`), and the
  recipe's own folder. A recipe that names no folder counts the whole repository (Heimdall, Sarina, ...). Commits elsewhere in
  the repository do not make a new build of a package they do not touch.

Recipes inside a repository source the helper (Yggdrasil's `packages/pkgbuilds/*`, this repository's `packages/pkgbuilds/*`);
the store's recipes (`profile/airootfs/usr/share/labyrinth/pkgbuilds`, copied onto every system and built from there) and the
apps' own recipes (Heimdall's and Limbo's `packaging/arch`, Huginn's `pkgbuild`) carry a copy of it. `tests/test_package_version.py`
keeps the copies here and in Yggdrasil the same.

A `-git` recipe builds from makepkg's clone of the app's repository, so its numbers come from that clone: at the top of the
recipe (makepkg reads the recipe again for the packaging step, when the clone is there), in `prepare()` (for the rest of the
build, once the clone is up to date) and in `pkgver()`. Its own folder is in another repository (here), so the recipe's commits
since the app's release count by date. `argus-git` also counts the firmware's commits since the app's release: either one
moving on is a newer package.

The publish (`labyrinth-publish-repo`, `packages/publish-repo`) publishes the newest build of each package unless the published one is the same program
at the same version; a release is always published, so every system shows the new version. If a build differs from the
published one but its version is not newer, the publish says so and keeps the published one: the recipe's `_version` line is
missing a folder the package is built from.

## Making a release

```
scripts/release.sh                      # every project's version, and what each package would be built as now,
                                        # next to what is installed (changes nothing)
scripts/release.sh heimdall patch       # 1.0.0 -> 1.0.1: a fix release of Heimdall
scripts/release.sh sarina minor         # 1.0.1 -> 1.1.0
scripts/release.sh system 1.1.0         # Labyrinth OS 1.1: this repository, Yggdrasil and the medium's os-release
```

PROJECT is `system`, `heimdall`, `limbo`, `sarina`, `kairos`, `argus` (the app), `daedalus`, `huginn` or `icarus`; the version
is `patch`, `minor`, `major` or an exact `X.Y.Z`. The script checks the new version is newer and that `VERSION` has no
uncommitted change, writes `VERSION` (for the system also Yggdrasil's and `profile/airootfs/etc/os-release`; for Sarina also
the version in `packaging/arch/PKGBUILD`, its recipe for a released tarball, and in `go/internal/version/version.go`, see
below), commits only those files ("Release X.Y.Z"), tags the commit `vX.Y.Z` and prints the `git push` that publishes it. It
pushes nothing and builds nothing: the next publish after the push builds every package of that project as `X.Y.Z-1`, and
installed systems update to it. For the system, build a new ISO after the push; its name carries the version.

## What the programs say

Wherever a program names its version (`heimdalld version`, `limbo version`, `sarina version`, About, a User-Agent, an MCP
`serverInfo`, a JSON `version` field), it is its project's `VERSION`: the system's for Yggdrasil and Labyrinth programs, its
own for an app. The programs read the file when they are built, so a release changes nothing else:

- **Rust**: `include_str!` of the repository's `VERSION` (for example `include_str!("../../../VERSION").trim_ascii()`). The
  `version` in each `Cargo.toml` is not used for it and is left as it is.
- **Go** (Heimdall, Limbo): `version.go` at the top of the repository embeds `VERSION` (`//go:embed`), and the commands use
  its `Version`. No recipe passes anything in.
- **Sarina's Go**: its module (`go/`) cannot embed a file above it, so `go/internal/version/version.go` keeps the number as a
  constant; a test holds it equal to `VERSION`, and `scripts/release.sh` writes both.
- **The Argus app**: `qflipper_common.pri` reads `VERSION` into `APP_VERSION` (the window, `qFlipper-cli --version`, the
  Chymaera API); `argus-tools` reads it as the Rust programs do. The firmware keeps its own `ARG-...` names.

Where Go and Rust builds of a program are compared (Heimdall's and Limbo's differentials), both read the same file, so they
print the same version. Today: `heimdalld 1.0.0`, `limbo version` 1.0.0 and `limbo rs-version` "limbo-rs 1.0.0 (...)",
`sarina 1.0.0 (commit ...)`, the Yggdrasil panel's notification server and `yggdrasil-weather/1.0.0`, Icarus's MCP server,
Daedalus's STEP header ("Daedalus CAD 1.0.0") and the Argus app (1.0.0). Labyrinth's own crates, Kairos and Huginn print no
version of their own.
