# Release checks

`scripts/release-check.sh` runs everything that can be checked without root, a VM or a network install (about 8 minutes;
`scripts/release-check.sh quick` skips the Rust tests and dependency audit). It exits 1 if anything fails.

What it checks: the generated package list matches `packages.base` + `components.json`; shell and Python scripts parse; shellcheck (when
installed); every systemd unit verifies; every file `profiledef.sh` lists exists; every PKGBUILD parses; all Python and shell tests;
`cargo test` for each Rust crate; `cargo audit` on each `Cargo.lock` (install: `cargo install cargo-audit --locked`).

## Found and fixed by it (2026-10-16)

- The generated `packages.x86_64` had lost five base packages (yggdrasil-idle, yggdrasil-keyring-sync, gnome-keyring, clamav, python-dbus):
  they had been added to the generated file instead of `profile/packages.base`, so the next regeneration would have dropped them from the ISO.
- `labyrinth-appnet-ask.service` had an invalid `ProtectControlGroups=` value (systemd ignored it).
- `yggdrasil-weather` used `rustls` 0.23.45 (RUSTSEC-2026-0346, TLS secrets visible in Debug output): updated to 0.23.46.

## Known warnings (not vulnerabilities)

`cargo audit` reports "unmaintained" for `paste` and `ttf-parser`, pulled in by the GUI libraries (egui and friends). Nothing to fix on our
side; they go away when those libraries move on. Re-run the audit before each release.

## Not done yet (needs root and time: ask the owner)

1. Build the ISO: `sudo scripts/build-local-packages.sh && sudo scripts/build.sh` (needs archiso, the CachyOS keyring and about 15 GB).
2. Boot it in a VM (`scripts/vm`), run the Calamares installer, reboot, then `labyrinth-selfcheck` and the checklists in `docs/STATUS.md`.
3. Secure Boot enrolment and the encrypted-disk path (`docs/INSTALLER.md`) have not been exercised.
4. shellcheck is not installed on this machine (`sudo pacman -S shellcheck`).
5. A CI workflow that runs `release-check.sh quick` on every push would catch things like the packages.base slip; not set up.
