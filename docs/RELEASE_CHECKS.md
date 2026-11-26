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

## Checks from the reference plan (in my private plans)

- **No analysis network reaches the home connection (R8):** Limbo's `sh scripts/netns-sinkhole.sh` (no root; Limbo's CI runs
  it on every push): a guest on `sinkhole` or `none` gets zero packets out; `internet-only` is the control that must leak.
- **Every data file has its licence and notice (R1, R3):** Heimdall's `NOTICE-ATTACK` (MITRE ATT&CK) is installed; the vendor
  lists come from Arch's hwdata.
- **Nothing observed is sent off the machine unless its switch is on (R11):** no check yet; R5 and R6 were dropped.

## What it cannot check

Building the ISO (`labyrinth-publish-repo build && sudo scripts/build.sh`: the first as a normal user, since makepkg refuses
root; needs archiso, the CachyOS keyring, the Labyrinth signing key or `LABYRINTH_UNSIGNED=1`, and about 15 GB), booting it in a
VM (`scripts/vm`), installing (`yggdrasil-installer` with `yggdrasil-install-system`), and after the reboot `labyrinth-selfcheck`
and the checks in [FIRST_BOOT.md](FIRST_BOOT.md) §6. The encrypted-disk install has been run on real hardware, beside Windows;
Secure Boot enrolment has not been exercised. Two install paths only a VM run shows: an install with the image copied into
memory at boot (the boot menu's "run from memory" entry, `copytoram=y`; the other entries start the desktop straight from
the stick, `copytoram=n`, rather than wait for archiso to read the whole image into memory; the installer then copies `/run/archiso/copytoram/airootfs.sfs` and keeps the
stick off its disk list), and an install with a browser ticked that the medium lacks (Firefox): the new system's pacman takes
the lists the ISO was built from (`/usr/share/labyrinth/sync`, copied into the live system's `/var/lib/pacman/sync` at start)
and the live keyring, and installs it without a full upgrade.

## Done

- shellcheck is installed on the build PC, so the local run checks shell scripts too.
- `.github/workflows/checks.yml` runs `release-check.sh quick` and the Rust tests on every push and pull request.
