# CLAUDE.md: Labyrinth OS

Guidance for Claude Code (and any AI agent) working in this repository, in the layout every Artifex repository's CLAUDE.md
follows: what this is, build and test, rules, contracts, open work, know-how.

## 1. What this is

Labyrinth OS is a CachyOS-based daily and gaming distribution with Yggdrasil as its desktop. This repository holds the
archiso profile (`profile/`), Labyrinth's own Rust programs (`packages/tools` is `labyrinth-tools`, plus `distress`,
`distress-pam` and `publish-repo`), the recipes of the signed `[labyrinth]` package repository (`packages/pkgbuilds/`), the
build and release scripts (`scripts/`) and the tests (`tests/`). What the OS is today: [docs/STATUS.md](docs/STATUS.md); the
design: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). The desktop's code is in the Yggdrasil repository, checked out next to
this one (`../Yggdrasil`, or `YGGDRASIL_DIR`).

## 2. Build and test

Every cargo run under the memory cap, one crate at a time:

```bash
CAP="systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0"
cd packages/tools && $CAP env CARGO_BUILD_JOBS=4 nice -n 19 cargo test        # likewise distress, distress-pam, publish-repo
cargo fmt --check && $CAP env CARGO_BUILD_JOBS=4 nice -n 19 cargo clippy --all-targets -- -D warnings
scripts/release-check.sh quick      # no root: package list, scripts parse, shellcheck, units, profiledef, PKGBUILDs, Python and shell tests
scripts/release-check.sh            # also every crate's cargo test and cargo audit (about 8 minutes)
python3 tests/test_NAME.py          # one Python test; bash tests/test_NAME.sh for a shell one
```

The ISO (root, about 15 GB; the owner runs it): `scripts/build-local-packages.sh` as a normal user (makepkg refuses root;
`LABYRINTH_UNSIGNED=1` without the signing key), then `sudo scripts/build.sh`; `scripts/vm` boots it. What only that run can
check is in [docs/RELEASE_CHECKS.md](docs/RELEASE_CHECKS.md).

CI (`.github/workflows/checks.yml`, every push and pull request): `release-check.sh quick` against a pinned Yggdrasil
checkout, and for every crate in `packages/` fmt, clippy `-D warnings`, the tests and `cargo audit`, then the Python and
shell tests of labyrinth-tools' programs on that debug build. Watch it by the full commit hash (`gh run list --commit SHA`).

## 3. Rules

- **Push every commit as you make it**; do not ask first. Pull (`git pull --rebase`) before committing, stage by explicit
  path, and check the diff names no host name, network address, SSID, BSSID, MAC address, serial, account name or the
  owner's name, and no private repository. Docs are written in the first person as the owner ("I decided"), and say
  "Sarina", not "her".
- **Plans, owner steps, checklists, Sarina's eval reports and other internal notes** go in `docs/plans/` (private and
  git-ignored; ask the owner), never in the public docs. Public docs say what is built, how to use, build and test it, and
  the proof; a short "Known limitations" section may describe a limit, never plan it.
- **No root, no package installs, no service restarts and no reboots** from a session unless the owner asks for that step;
  things only the owner can do (sudo, BIOS, hardware, sign-ins) are written as steps for the owner.
- **Everything of ours that can be Rust is Rust** (security, then stability, then speed); shell and Python stay only where
  the platform needs them ([docs/RUST_PORT_REMAINING.md](docs/RUST_PORT_REMAINING.md)). Copies kept for a rollback go one
  week after the switched package runs on the owner's PC. No dead code.
- **Light**: nothing resident that is not asked for; it must run on modest hardware, down to a Raspberry Pi for Yggdrasil.
- **The base system is not ours**: pacman, the AUR, the Arch and CachyOS repositories and their tooling stay as they are;
  the `[labyrinth]` repository is listed last and never replaces a base package, except coreutils and sudo, which it swaps
  for their packaged Rust versions ([docs/RUST_BASE.md](docs/RUST_BASE.md)); no others.
- **The GPU unlock** follows the safety rules in [docs/GPU_UNLOCK.md](docs/GPU_UNLOCK.md); nothing touches a graphics card's
  clocks, power or firmware outside them.
- **A change another repository relies on** (a recipe's paths, the package repository, an integration file, the voice unit,
  Heimdall's template) updates [docs/CONTRACTS.md](docs/CONTRACTS.md) in the same commit, is flagged `CONTRACT:`, and gets a
  handoff note in `docs/plans/LOG.md` (private).

## 4. Contracts

- What other repositories rely on from Labyrinth: [docs/CONTRACTS.md](docs/CONTRACTS.md) (the `[labyrinth]` package
  repository, the recipes built from the sibling repositories, `scripts/package-version.sh`, the desktop's integration files,
  Sarina's voice unit and settings, Heimdall's configuration template, the agent account).
- What Labyrinth uses: Sarina's package inputs, voice socket and programs (Sarina's `docs/CONTRACTS.md`); Yggdrasil's
  integration-file format, installer and Settings; Heimdall's and Limbo's packaging and configuration (their docs).

## 5. Open work

docs/plans/BACKLOG.md and docs/plans/LOG.md (private; ask the owner).

## 6. Know-how

- **The package list is generated**: edit `profile/packages.base` and `profile/airootfs/usr/share/labyrinth/components.json`,
  then `scripts/gen-packages.py` writes `profile/packages.x86_64`; never edit the generated file (the release check compares
  them).
- **Recipes**: every recipe sources `scripts/package-version.sh` and sets `_version`; a rebuild that must reach installed
  systems raises `pkgrel`. The store's recipes in `profile/airootfs/usr/share/labyrinth/pkgbuilds` are copied onto every
  system; keep them in step with `packages/pkgbuilds` where both exist (sarina-git has two copies). Versions and releases:
  [docs/VERSIONING.md](docs/VERSIONING.md), `scripts/release.sh`.
- **Publishing** (`labyrinth-publish-repo`) builds from fresh clones of each repository's `main` on GitHub, never from a
  working tree, so a change reaches the repository only once it is pushed. [docs/PACKAGE_REPO.md](docs/PACKAGE_REPO.md).
- **Programs on the image**: `profile/airootfs/usr/local/bin` holds the image's own names (many are links into
  `labyrinth-tools` or `sarina-voice`); labyrinth-tools' programs are one Rust binary picked by name (`packages/tools/src/bin`).
- **Out of the tree**: `out/` holds built ISOs; never commit it.
- **Hardware notes** per machine (firmware settings, what works): [docs/HARDWARE.md](docs/HARDWARE.md).
