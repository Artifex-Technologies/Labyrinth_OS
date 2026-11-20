# Labyrinth OS's contracts with the other repositories

What other projects rely on from this repository, one section each: what it is, who uses it, and where it is defined. A
change to any of these is a cross-repository change: update this file in the same commit, flag the commit `CONTRACT:`, and
tell the other side.

## The `[labyrinth]` package repository

- **What:** a signed pacman repository on the GitHub release named `packages`
  (`https://github.com/Artifex-Technologies/Labyrinth_OS/releases/download/packages`), listed last in `pacman.conf` with
  `SigLevel = Required`; every package and the database are signed with the Labyrinth key (`labyrinth-keyring`). A package of
  the same name in Arch's or CachyOS's repositories always wins. It replaces two base packages, on every system that
  uses it (LykanOS too): `coreutils` with `labyrinth-coreutils-rs` and `sudo` with `labyrinth-sudo-rs`, their packaged Rust
  versions ([RUST_BASE.md](RUST_BASE.md)); no others.
- **Who uses it:** every installed Labyrinth system and Agora's updates; LykanOS (its ISO and installs take Sarina, Heimdall,
  Limbo and the desktop from it); Limbo's analysis image (`yara-x`, fetched on the host).
- **Where:** `docs/PACKAGE_REPO.md`; `packages/publish-repo` (`labyrinth-publish-repo`); `packages/pkgbuilds/labyrinth-keyring`;
  `profile/pacman.conf`.

## The reading fonts (`ttf-atkinson-hyperlegible-next`, `-mono`, `ttf-lexend`, `otf-opendyslexic`, `-mono`)

- **What:** five font packages built in this repository and published in `[labyrinth]`, by these names, with their files where
  Yggdrasil's `packages/yggdrasil-conf/src/fonts.rs` looks for them (`/usr/share/fonts/TTF/...`, `/usr/share/fonts/OTF/...`).
  The sixth, `ttf-atkinson-hyperlegible`, is Arch's.
- **Who uses it:** Yggdrasil: `yggdrasil-theme` depends on `ttf-atkinson-hyperlegible` and `ttf-atkinson-hyperlegible-mono`
  (the default system font, its `58-yggdrasil-default-font.conf`), `yggdrasil-settings` on all six (Appearance > Fonts), so
  an update brings them to every installed system; LykanOS through the same packages.
- **Where:** `packages/pkgbuilds/<name>`; `docs/PACKAGE_REPO.md`, "the reading fonts".

## The recipes built from the sibling repositories

- **What:** `packages/pkgbuilds/<name>` builds each project from its own repository's `main` (never a working tree):
  `sarina-git`, `heimdall-git` (and `-multilink-git`, `-private-mic-git`, `-yara-check-git`), `limbo-git`, `kairos-git`,
  `argus-git`, `daedalus-cad-git`, `icarus-git`; the paths, binaries and run-time dependencies each recipe expects are that
  project's packaging contract (Sarina's is in its `docs/CONTRACTS.md`). The store's copies are in
  `profile/airootfs/usr/share/labyrinth/pkgbuilds`.
- **Who uses it:** Sarina, Heimdall, Limbo, Kairos, Argus_App, Daedalus_CAD and Icarus (their layouts must keep building);
  LykanOS (installs the results).
- **Where:** `packages/pkgbuilds/`; `docs/PACKAGE_REPO.md` "Publishing".

## Versions (`scripts/package-version.sh`)

- **What:** the helper every recipe sources for `_version` (the project's `VERSION` plus the build number in `pkgrel`);
  copies of it live in the other repositories' recipes and must stay the same.
- **Who uses it:** Yggdrasil (`scripts/package-version.sh`), Heimdall's and Limbo's `packaging/arch`, the store's recipes.
- **Where:** `scripts/package-version.sh`; `docs/VERSIONING.md`; `tests/test_package_version.py` keeps the copies equal.

## Integration files for the desktop

- **What:** `/usr/share/yggdrasil/integrations/labyrinth-*.conf` (app rules, branding, installer, packages, security): how
  Yggdrasil's panel, Settings, Agora and installer show and drive Labyrinth's parts, in the format Yggdrasil defines.
- **Who uses it:** Yggdrasil (Settings, Agora, the installer, the panel).
- **Where:** `profile/airootfs/usr/share/yggdrasil/integrations/`.

## Sarina's voice unit and settings

- **What:** the user unit `labyrinth-voiced.service` starts `/usr/bin/sarina-voiced` (Sarina's voice service); the image's
  `/usr/local/bin` names (`labyrinth-ask-sarina`, `labyrinth-sarina-checkup`, `labyrinth-sarina-speak-voiced`) link to
  `/usr/bin/sarina-voice`; Settings > Assistant's switches are written to `~/.config/labyrinth/sarina.env`
  (`labyrinth-sarina-setting`).
- **Who uses it:** Sarina (her programs read the unit's environment and `sarina.env`), Yggdrasil's talk key.
- **Where:** `profile/airootfs/usr/lib/systemd/user/labyrinth-voiced.service`, `profile/airootfs/usr/local/bin/`,
  `packages/tools/src/sarina_setting.rs`; `docs/SARINA_VOICE.md`.

## Heimdall's configuration template

- **What:** the default `/etc/heimdall/config.json` Labyrinth installs, including the file-integrity list (`fim_paths`: the
  Sarina, Limbo and Heimdall binaries and units) and the agent session folder (`/var/lib/sarina/sessions`).
- **Who uses it:** Heimdall (reads it); Sarina (her session folder must match).
- **Where:** `packages/tools/src/text/heimdall-config.json`.

## The engineering agent's account (`labyrinth-agent`)

- **What:** `/usr/bin/labyrinth-agent`, the one program the `claude-agent` account may run as root, with its fixed verbs
  (`pkg-install`, `pkg-add`, `service`, `logs`, `maintenance`, `limbo`, `rollback`, `firewall-status`, `deploy`), each logged
  for Heimdall.
- **Who uses it:** Claude Code sessions on a Labyrinth system; Heimdall (reads the log, attributes the actions).
- **Where:** `packages/tools/src/agent.rs`, `agent_setup.rs`, `agent_window.rs`; `docs/AGENT_ACCOUNT.md`.
