# Status (updated 2026-10-11)

## Verified (built and tested on Windows)

`packages/minos` (`cargo test`: 20 pass; `cargo build` clean):
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
`packages/minos` (22) and `packages/labyrinth-weather` (34).
Not done: everything that needs Linux or the real machine. See DESKTOP.md, COMPONENTS.md,
WEATHER.md, INSTALLER.md, and the "what is missing for an ISO" list below.

### Missing before an ISO can be built
- `profile/pacman.conf` has no `[labyrinth]` local repo for the packages we build ourselves
  (calamares, minos, labyrinth-weather, proton-pass, visual-studio-code-bin).
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
  `minos` successfully; checksums verified; packages inspected.
- `labyrinth-weather` run live on Linux; `labyrinth-apps`, `probe.sh`, `labyrinth-gaming check`
  and `labyrinth-displays` (clean failure without Hyprland) run for real.
- `minos` run as a real window under WSLg and driven with mouse and keyboard:
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
- Local `[labyrinth]` repo built by `scripts/build-local-packages.sh`: calamares, labyrinth-minos,
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

### Where things stand (2026-10-11, end of session)

Latest ISO: `C:\Users\grego\Downloads\Labyrinth-OS\labyrinth-2026.10.11-x86_64.iso` (6.0 GB, sha256
`e19469a5...cfaf246b6`, in SHA256SUMS next to it). Built from the working tree; **nothing from
2026-10-11 is committed yet** (the GitHub repo Artifex-Technologies/Labyrinth_OS has the earlier work).

Done on 2026-10-11 and verified in the VM (screenshots), unless marked otherwise:
- **Windows feel.** Window buttons on the right in Windows order (minimize, maximize, close), macOS
  look. Shortcuts: Win+E Minos, Win+R run, Win+I settings, Win+L lock, Win+V clipboard,
  Win+Shift+S screenshot, Ctrl+Shift+Esc task manager. Terminal commands `cls` and `ipconfig`
  (/all, /release, /renew, /flushdns) in the Windows layout.
- **Dock like a taskbar.** Maximized and new windows stop above it (KWin script
  `labyrinth-fit-windows`); Win+F full screen hides dock and bar; Win+Space brings them up over a
  full-screen app, Esc hides them. The Hyprland (Omarchy) session has no dock any more.
- **One screen macOS-style, one Omarchy-style** (DESKTOP.md has the full key table): the LG DualUp
  tiles with Krohnkite (`kwin-scripts-krohnkite`, our PKGBUILD), has its own workspaces 1-9 and a
  slim bar; keys change meaning by screen (KWin script `labyrinth-screen-keys`);
  `labyrinth-tiling OUTPUT|off`; labyrinth-displays turns it on for the DualUp once per port.
  Verified in a two-monitor VM at half size; **not yet on the real monitors**.
- **Faster, smaller install.** ~2 min in the VM (was ~6): resume hook added before the initramfs is
  built (no second `mkinitcpio -P`), one grub-mkconfig, keyring made on first boot. 15 GB installed
  / 6 GB ISO (was 21 / 10): Ollama+CUDA no longer in the image (Sarina offers `ollama-cuda` or
  `ollama-vulkan` on first use), COSMIC dropped, bore kernel dropped (cachyos + lts remain).
- **From CachyOS's installer, adopted into ours:** firewall on by default (ufw: deny incoming, allow
  outgoing; System Settings > Firewall) and a permanent "Fresh Labyrinth OS install" snapshot.
  Decision: keep our upstream Calamares build (cachyos-calamares collides with our config).
- **Cleanup:** duplicate gaming tools and Proton Drive unit removed; old VM scripts and the WSLg GUI
  test removed; already-built local packages are skipped (scripts/build-local-packages.sh).
- Daedalus CAD (separate repo) builds on Arch: 39 tests pass, `flatten` numbers hand-checked.

Later on 2026-10-11 (in the working tree, VM-tested unless noted; not yet in a copied ISO):
- Window frames: Klassy replaces WhiteSur's aurorae (traffic lights, title bar in the app's colour,
  no chin, invisible resize edges); Kvantum WhiteSurDark so KDE toolbars are dark. Minos draws its
  own title bar (chrome.rs). Konsole + Yakuake replaced by Ghostty (tabs in the title bar,
  traffic-light CSS, F12 drop-down pre-approved in kglobalshortcutsrc, systemd drop-in fixing
  "lacks handler for reload signal USR2" on systemd 262). Krohnkite floats portal/polkit dialogs.
- **Tile border (working):** four strips drawn as Plasma OSD dialogs around the focused tile
  (`labyrinth-tile-border`). The earlier failures were a stale QML cache: KWin keeps compiled script
  QML in ~/.cache/kwin/qmlcache across logins. `labyrinth-kwin-cache` (user service) now clears it
  whenever Labyrinth's KWin scripts change.
- **Top bar and dock** keep the translucent look with a purple tint (whitesur-kde-theme pkgrel 2,
  `labyrinth-plasma-colors`).
- **Labyrinth Settings (packages/settings + packages/labyrinth-ui):** Rust/egui, GNOME-style pages
  (network, Bluetooth, displays incl. the tiling-screen switch, sound, appearance, power, firewall,
  date and time, apps, shortcuts, about). KDE System Settings/Info Center and their ~72 entries are
  hidden (`labyrinth-hide-kde-settings`), `systemsettings MODULE` redirects to the matching page
  (`/usr/local/bin/systemsettings`), a few deep options open a single `kcmshell6` panel.
  Untested on real hardware: Wi-Fi/Bluetooth pages (the VM has neither), power profiles.
- Owner decisions: Ghostty stays (own terminal on hold); Settings app first. Open question put to
  the owner: replace Plasma's shell (top bar and dock) with our own Rust panel; keep KWin for now.

Still to do, most important first:
1. **Commit and push** today's work (ask first; the owner has not said to commit yet).
2. **Real hardware on the main PC** (i9-9900K, RTX 2080 Ti, Acer X34 GS + LG DualUp): boot the live
   USB, check NVIDIA, both monitors at the right mode/scale/position, the DualUp tiling screen and
   the per-screen keys, then install. Secure Boot off; BitLocker recovery key at hand.
3. **Tiling screen polish:** coloured border on the active tile (Omarchy style); check heavy apps
   (CAD, games) tiled; maybe an on/off toggle in the top bar.
4. Plasma lock screen in the "Split" style; SDDM logo not showing.
5. Minos as the file picker for all apps (xdg-desktop-portal FileChooser backend), owner approved.
6. Confirm on a screen: kwallet-pam (no wallet prompt at login), easier window-edge resizing,
   Yakuake colours.
7. Dual-boot support in the installer (BitLocker note); Claude Code on Linux; icon packs for Minos.
8. Optional: package Daedalus CAD as a component (like sarina-git) with .stl/.dxf file associations.

How to build and test (WSL distro `archlinux`): `scripts/build-local-packages.sh` as builder (copy new
PKGBUILDs to /home/builder/labyrinth/packages first), then `LABYRINTH_WORK_DIR=/root/isowork
LABYRINTH_OUT_DIR=/root/isoout scripts/build.sh` as root. VM: `scripts/vm/run-iso.sh new|iso|disk`;
two monitors with `VGA_ARGS="-device virtio-vga,id=vga0,xres=1720,yres=720 -device
virtio-gpu-pci,id=vga1,xres=1280,yres=1440"` (QEMU 11.1.2 crashes on per-screen `screendump`; take
screenshots in the guest with `spectacle -b -n -f`). Tests: `bash tests/test_desktop_scripts.sh`,
`bash tests/test_install_cleanup.sh`, `python3 -m unittest tests.test_displays tests.test_bar_tools
tests.test_labyrinth_apps tests.test_steam_playall`.

## Rust shell, calculator, installer (2026-10-12)
- **labyrinth-panel (packages/panel):** Rust top bar + dock (Wayland layer-shell, plasma-window-management for the task list). Opt-in: `labyrinth-panel-mode on|off|status` (Settings → Appearance → "Top bar and dock"). Missing for parity: system tray (StatusNotifier), global menu, quick-settings popover, launcher, notifications, fractional scaling, dock context menus/previews.
- **panel.conf (packages/labyrinth-conf):** `~/.config/labyrinth/panel.conf` (dock size 32-72, dock/bar opacity 0-100 where 0 = invisible, colour = match theme or plain dark/light). Settings writes it; the panel re-reads it about once a second.
- **labyrinth-calculator (packages/calculator):** standard pad plus history at >= 600 px, scientific pad at >= 860 px. The equation stays at the top and editable after "="; the result shows below. One decimal point per number. History persists.
- **Installer:** Calamares kept; branding `stylesheet.qss` uses the Minos/Settings palette. Needs a visual check in a VM install.
- Not yet verified in a VM: Settings dock controls end-to-end, the installer look, panel on real NVIDIA hardware.
- Candidates to rewrite next: launcher/KRunner, notification daemon, lock screen, SDDM login, screenshot tool, task manager, volume/brightness OSD, clipboard history.

## Icons (2026-10-12)
- System icon theme is **MacTahoe (purple)**, dark flavour, via package `mactahoe-icon-theme` (replaces WhiteSur icons). KDE/GTK use the overlay theme **Labyrinth** (`/usr/share/icons/Labyrinth`, inherits MacTahoe-purple-dark) so per-app overrides work everywhere.
- Minos now draws folders, files, drives, Home and This PC from the theme (`labyrinth-ui::sysicon`); folders with a colour tag keep the tinted drawn icon, and every icon falls back to the old vector one.
- `labyrinth-icon APP FILE`, `labyrinth-icon --import FOLDER`, `labyrinth-icon --reset APP`: give apps other icons (png/svg/macOS .icns; `labyrinth-icns` extracts the PNG from an .icns). Used for the dark macOS 27 icons from macosicons.com, which are not redistributed with the ISO.
- Not done: Settings' own sidebar icons are still hand-drawn; icon theme does not switch to a light flavour with the light theme; the Golden Gate flat set has no public pack.

## Rename (2026-10-12)
The file explorer is now **Minos** (was Finder, and Labyrinth Files before): `packages/minos`, package `labyrinth-minos`, binary `minos`, config folder `~/.config/minos` (reads fall back to `finder` and `labyrinth-files`). `/usr/bin/finder` stays as a link, and pins named `finder` in the dock file still work. The macOS Finder icon (a dark macOS icon you may import) is unaffected: `labyrinth-icon` still matches files named `Finder_...` and `Minos_...`.

## Dock (2026-10-12)
`panel.conf` has `dock_pos` (bottom, left, right) and `dock_gap` (0 to 40 px, default 4; 0 is right on the edge); Settings > Appearance > Dock has both. The Trash is the last dock item (empty or full; opens the trash folder in Minos).

How it is placed (found by testing in the VM): KWin lays a layer-shell surface out above any room reserved on its edge, including its own exclusive zone, so a layer-shell dock floated off the edge by its own height, and negative margins or zone -1 did not help. The dock is therefore a **Plasma panel window** (xdg toplevel with the `org_kde_plasma_surface` panel role and an explicit position), which KWin puts exactly where it is told. KWin takes no strut from such a window in Plasma 6, so an invisible layer-shell strip (`Kind::Strut`, Background layer, anchored along the edge) reserves the room; maximized windows stop at the dock. The top bar stays a layer-shell surface (flush at the top).

## Minos Storage (2026-10-12)
Storage lists the physical disks (`lsblk`), with one usage bar each; selecting a disk shows its partitions (`Disk:vda` is a virtual location like This PC). A partition that is not mounted is listed but cannot be opened yet. The computer's name replaces "This PC" once it is set. This PC, My User Home and the drives use Minos' own drawn icons; folders and files use the MacTahoe theme.

## Installer (2026-10-12)
Title bar: the installer runs as root and does not read the user's colour scheme, so KWin drew it grey. The live image ships `LabyrinthInstaller.colors` and a KWin window rule (`~/.config/kwinrulesrc`) that gives the Calamares window that scheme, so the title bar blends in. The slideshow gets a dark background and says Plasma, not Hyprland.

## Boot menu (2026-10-12)
GRUB uses grub-evodevo (MIT, vendored in `usr/share/labyrinth/grub-evodevo`) through `labyrinth-grub-theme`: black background, Labyrinth purple, 1920x1080, an emblem per system. `data/Labyrinth` is the logo as a 24x24 emblem, sized like the CachyOS one. The installer's boot step (2e) runs it last and puts the old menu back if the result has no entries. `os-prober` is installed and enabled for dual boot. Verified in a VM install: the menu shows "Labyrinth Linux" with the emblem, Advanced options, UEFI Firmware Settings (gear) and the other firmware boot entries; the entry boots to the login screen. Not tested with a real Windows disk, on real hardware, or at other GRUB resolutions (it asks for 1920x1080 and falls back to auto).

## Top bar, pill style (2026-10-12)
`labyrinth-panel`'s bar has no strip of its own now: floating pills over the wallpaper (their backdrop is Settings > Appearance > Top bar > Background; 0% leaves floating text). Left: the Labyrinth button, the desktops (1..9 with a "+"; click to switch, "+" adds a plain desktop), the focused app. Middle: a pure-black tab where the notch will open (inert for now). Right: network and Bluetooth icons (read with nmcli and bluetoothctl on a worker thread), the date ("Sun 19 Dec") and the time in 24 hours. The quick power button is gone (its job moves to the notch's Power tab). Not done yet: the start-menu dropdown, the workspace profile chooser, weather, the notch itself. Design notes: see docs in the memory note "Bar and workspaces design".
