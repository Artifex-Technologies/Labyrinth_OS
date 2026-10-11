# Status (2026-10-10)

## Verified (built and tested on Windows)

`packages/finder` (`cargo test`: 20 pass; `cargo build` clean):
- Core logic: history/tabs, listing + sort + filter, file operations
  (copy/move/rename/new folder, never overwrite, refuses folder-into-itself),
  persistent state (tabs, favorites, tags, colors, view; atomic save, corrupt
  file falls back to defaults), background recursive search, mount parsing,
  Proton Drive link verification logic.
- The egui app builds, opens, and writes `state.json` on exit (session restore
  data). `labyrinth-cloud` CLI builds and handles arguments.
- `tests/test_steam_playall.py` (Steam config editor): 7 tests pass.

## Written but never exercised

- The explorer's **interactive behavior**: nobody has clicked through it. That
  covers drag and drop, context menus, selection, split pane, gallery thumbnails,
  rename dialog, live updates, and the Cloud badges. Thumbnails decode full-size
  images, so very large photo folders may be slow or memory-hungry.
- Trash on Linux (the `trash` crate), drive list from `/proc/mounts` (parser
  tested, real mounts not), udisks2/MTP (not implemented).
- `labyrinth-gaming` shell script (syntax-checked only), `proton-drive.service`,
  `probe.sh`, the archiso profile and all package names.
- Verification against a real Proton Drive remote (real rclone output on local folders: tested).
- WSL has no distribution installed, so none of the Linux-side pieces have run.

## Installer (see INSTALLER.md)

Calamares config generated from the real 3.4.2 samples, YAML-checked; the cleanup
script is tested against a mock root (20 checks); bootfinish is syntax-checked
only; nothing has been installed or booted.

## Not built yet

Folder-size toggle, multi-column sort, font picker, interface languages,
benchmarking tools, raw/unmounted drive detection, phone (MTP) support, network
devices, Sarina hook, a desktop in the live image (Hyprland/SDDM), Calamares branding, most PKGBUILDs, Limbo `driver-test` profile.

## Desktop, components, weather, displays (2026-10-10)

Tested here: `tests/test_labyrinth_apps.py` (20), `test_bar_tools.py` (7), `test_displays.py` (13),
`test_desktop_scripts.sh` (16), `test_install_cleanup.sh` (20); `cargo test` in
`packages/finder` (22) and `packages/labyrinth-weather` (34).
Not done: everything that needs Linux or the real machine. See DESKTOP.md, COMPONENTS.md,
WEATHER.md, INSTALLER.md, and the "what is missing for an ISO" list below.

### Missing before an ISO can be built
- `profile/pacman.conf` has no `[labyrinth]` local repo for the packages we build ourselves
  (calamares, finder, labyrinth-weather, proton-pass, visual-studio-code-bin).
- No `scripts/build.sh` or `scripts/build-local-packages.sh` (Chymaera has these to port).
- No live-session setup in `airootfs`: live user, SDDM + session files, os-release, hostname,
  locale, `efiboot`/bootloader entries. The profile is a skeleton.
- CachyOS keyring and mirrorlist provisioning for the build.

## Linux (Arch in WSL) verification, 2026-10-10

Ran on real Arch Linux under WSL2 (no GPU session, so no Hyprland, installer or boot):
- All script/Python suites and both Rust suites pass (`scripts/wsl-test.sh tests`).
- Package names: all found in Arch's repos (core/extra/multilib) except CachyOS's own
  (`linux-cachyos*`, `proton-cachyos`), `protonup-qt` (AUR), and our own packages.
- `makepkg` built `proton-pass`, `visual-studio-code-bin`, `labyrinth-weather` and
  `finder` successfully; checksums verified; packages inspected.
- `labyrinth-weather` run live on Linux; `labyrinth-apps`, `probe.sh`, `labyrinth-gaming check`
  and `labyrinth-displays` (clean failure without Hyprland) run for real.
- `finder` run as a real window under WSLg and driven with mouse and keyboard:
  navigation, hidden files, tabs, split pane, gallery thumbnails, rename, copy/paste,
  cut/paste, new folder, delete to the freedesktop Trash all verified on the real filesystem.
  Bugs found by this and fixed: Wayland app id contained a space (WSLg dropped the
  connection), Enter in the Rename dialog did nothing, Ctrl+C/X/V shortcuts never fired,
  and arrow glyphs drew as empty boxes.
- GUI checks now run on a virtual display (`scripts/xvfb/xv.sh`), which never takes over the
  screen; the old WSLg script (`tests/gui_smoke_wsl.*`) was removed.
- Native Wayland run of the explorer works under WSLg (after the app-id fix) with software
  rendering; GPU rendering is untested.

## ISO build and first boot (2026-10-10)

Done, on Arch under WSL:
- Local `[labyrinth]` repo built by `scripts/build-local-packages.sh`: calamares, labyrinth-finder,
  labyrinth-weather, proton-pass, visual-studio-code-bin all build.
- `scripts/build.sh` builds the ISO with mkarchiso (CachyOS keyring and mirrorlists provisioned on
  the host; the generated package list resolves against the CachyOS v3 repos). Result: a ~9.7 GB
  UEFI ISO with the "everything" component preset (it is big because of cosmic, cuda/ollama, qemu,
  wine; trim with `scripts/gen-packages.py --components ...`).
- `scripts/vm/run-iso.sh` boots it in QEMU with OVMF; `scripts/vm/serial.py` runs commands in the
  live system over a serial root shell (live medium only).
- The live ISO boots: CachyOS kernel 7.2.9, systemd reports `running` with no failed units, live
  user autologs into Hyprland, bar and dock are up, Calamares starts with admin rights and
  reaches the install step on an erase-disk layout (1 GiB EFI + Btrfs). One earlier boot of an
  otherwise identical ISO stopped in the initramfs ("no root file system image found"); it did not
  repeat. Not explained yet.
- Hibernate: `labyrinth-install-bootfinish` creates a RAM-sized swap file (nested btrfs subvolume),
  resume parameters and hook. Not yet confirmed by an install.
- `labyrinth-opacity` (dock/bar opacity), bar layout change, installer branding and the logo set (`scripts/gen-logo.sh`, from `assets/logo/labyrinth-source.jpg`).

### First full install in a VM (2026-10-10): what it found

Install (Calamares, erase disk) -> reboot -> GRUB -> kernel -> SDDM -> Hyprland desktop works in
QEMU/UEFI. Bugs the test found, all fixed in the tree:
- `squashfs-tools` missing from the image (unpackfs failed), `python-toml` missing (displaymanager
  failed), no active pacman mirrors, no pacman keyring on the live image (pacman-init.service added).
- **GRUB could not boot the LTS kernel**: GRUB's btrfs driver reads zstd-compressed files whose size
  is not a multiple of 4096 512 bytes short ("premature end of file"). Reproduced with `grub-fstest`.
  Fix: `labyrinth-install-cleanup` marks `/boot` NOCOW (so never compressed) before kernels are written.
  This would have hit the real machine too.
- `resume=` never reached the kernel command line (GRUB file uses single quotes); GRUB default entry
  was the LTS kernel; timeout was 5 not 2. Fixed in `labyrinth-install-bootfinish`.
- grub-btrfsd needs `inotify-tools`; `hostname` needs `inetutils`; the live account lingered on the
  installed system; the login screen defaulted to COSMIC. All fixed.
- The VM screenshot of a GRUB graphical menu is stale while GRUB shows an error ("Press any key"):
  it looked like a hang. Use a text console / serial to see GRUB errors.
Still unconfirmed on a real install: the hibernate resume parameters (the test disk was patched by
hand), snapshot entries in the GRUB menu, NVIDIA, the two real monitors.

### Real hardware (2026-10-11)

On a Dell OptiPlex 3080 (i5, 8 GB, 2 x 256 GB, Intel graphics), installed from the USB:
- The live session boots (DD-mode USB; ISO-mode in Rufus does not work), Wi-Fi sets up, the
  installer opens (once the launcher was fixed; see `labyrinth-install`), the system installs.
- **Hibernate works end to end** (power off, power on, session restored). The VM reset after
  resume seen in the nested-QEMU test was an artifact of that environment.
- Found on hardware: the dock covered the bottom of other windows (fixed: `-x`), the refresh rate
  stayed at 30 Hz on a monitor that can do 120 Hz (fixed: `labyrinth-displays` picks the best
  mode), tapping SUPER did nothing (to do).
- Direction change: the macOS-style desktop becomes KDE Plasma with the WhiteSur (SmallSur) look;
  Hyprland stays as the Omarchy-style tiling mode, chosen at login (`labyrinth-switch-mode`).

### Where things stand (2026-10-11, evening)

Done and seen working (VM screenshots): Plasma macOS-style desktop (WhiteSur look, traffic-light buttons
on the left, global menu in the top bar showing the focused app's menus, floating dock with hover
previews, Meta key opens the launcher), the Labyrinth installer inside it, a full install, hibernate
on real hardware (OptiPlex), Hyprland tiling session with floating-by-default windows.

Built since but not yet confirmed on a screen: the arrowhead logo set (`scripts/gen-logo.sh`) in the
installer, dock launcher and login screen; the login-screen background fix; no Apple logo in the top
bar; live session no longer locks or blanks during an install; Yakuake without its first-run dialog;
USB/Bluetooth power-saving off for wireless keyboards; best monitor mode on KDE (`labyrinth-displays
--kde`, tested against a fake kscreen-doctor and, in the VM, picks the largest 60 Hz+ mode).

Still to do, in the order that matters for daily use:
1. Boot the next ISO on the OptiPlex: login page, first Plasma login, switching to tiling and back.
2. Plasma lock screen in the "Split" style (hyprlock Split exists for the tiling session).
3. App name in bold next to the menus in the top bar; media controls, weather in the Plasma tray.
4. NVIDIA and the two real monitors (your main PC): boot the live USB there before installing.
5. Finder: icon packs; install-time component chooser; remaining ISO gaps (docs/ARCHITECTURE.md).
