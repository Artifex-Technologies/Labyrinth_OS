# Status (updated 2026-11-07)

What Labyrinth OS is today, what is in progress, and what comes next. The older dated entries (the Windows and WSL builds,
Plasma, SDDM, Calamares) are in this file's git history: `git log -p -- docs/STATUS.md`.

## What it is today

- **Base:** CachyOS's `x86-64-v3` repositories, the `linux-cachyos` kernel with `linux-cachyos-lts` as the fallback, NVIDIA's
  open modules. Built with archiso from `profile/` ([ARCHITECTURE.md](ARCHITECTURE.md)).
- **Desktop:** Yggdrasil, in its own repository ([Artifex-Technologies/Yggdrasil](https://github.com/Artifex-Technologies/Yggdrasil)),
  as a session on Hyprland 0.57. Top bar, dock, Fylgja (the notch), Settings, Agora (the app store), Minos (files), the lock
  screen, notifications, the screenshot tool, the clipboard history, the colour picker, the video player and spell check. No KDE:
  Plasma, KWin and SDDM left on 2026-10-24; the old Omarchy-style tiling desktop (waybar, mako, fuzzel and the rest) on 2026-11-05.
- **Login:** greetd with Yggdrasil's greeter. On the live medium greetd signs the live user in by itself (`/etc/greetd/live.toml`,
  which the install removes).
- **Installer:** Yggdrasil's installer window (`yggdrasil-installer`) with Labyrinth's install program (`labyrinth-install-system`).
  Erase a disk or choose partitions by hand (beside Windows, keeping its EFI partition); LUKS2 encryption on by default; btrfs with
  snapper and grub-btrfs; GRUB with Labyrinth's theme. Calamares was removed on 2026-11-07 (df5b5a2); there is no fallback installer.
- **Software:** the ISO stays under 4 GB. Vivaldi is the one browser on it. The installer's Browsers page adds others; its Optional
  software page offers Sarina, Heimdall and device software, which install at the first login, online. Agora has the rest
  ([COMPONENTS.md](COMPONENTS.md)).
- **Updates:** Labyrinth's signed package repository (GitHub release "packages", [PACKAGE_REPO.md](PACKAGE_REPO.md)) keeps an
  installed system current from any ISO. Since 2026-11-07 the ISO build's own packages are signed and checked too.
- **Security:** Heimdall, Limbo, the hardening defaults, the firewall (ufw, incoming blocked), the distress password, a network
  switch per app. Sarina is optional. See [SECURITY_ROADMAP.md](SECURITY_ROADMAP.md).

**Seen working:** the ISO builds and boots (last built 2026-10-26); the installer completes on real hardware, also beside Windows
and encrypted; the desktop runs on my main PC every day. `scripts/release-check.sh` runs every check that needs no root
([RELEASE_CHECKS.md](RELEASE_CHECKS.md)); GitHub runs its quick part on every push. That run fails today on two shell tests
(`test_install_cleanup.sh`, `test_labyrinth_rollback.sh`), which fail on this PC too; not looked into yet.

## In progress

- Sarina's night training: `sarina-night.timer`, 22:30 to 04:20. Reports in `docs/sarina-evals/`, wrap-ups in
  [TEAM_BOARD.md](TEAM_BOARD.md).
- Frame generation for games: Labyrinth's own Vulkan layer (`labyrinth-framegen`, a preview; the name is a working name), with
  lsfg-vk as the stopgap. Settings > Games chooses the games.
- The GPU unlock ([GPU_UNLOCK.md](GPU_UNLOCK.md)): Phases 0 and 1 are done. Phase 2 waits on my go, four BIOS readings and a game
  to measure with.
- Niflheim OS (formerly Chymaera OS): its repository was renamed on 2026-11-07
  ([Artifex-Technologies/Niflheim_OS](https://github.com/Artifex-Technologies/Niflheim_OS)) and it is moving to Yggdrasil.
- Settings > Wi-Fi & Network: Heimdall's list of devices on the network and its Wi-Fi checks (through the labyrinth-security
  integration).

## Next

1. A clean ISO build from scratch: the first one without Calamares and with signed local packages. Then the install check in
   [GO_LIVE.md](GO_LIVE.md) §2: no live-user sudo or autologin carried over, ufw on.
2. The rest of GO_LIVE §1: a way back (a snapshot after install, the GRUB snapshot menu, a rescue USB).
3. Privacy (GO_LIVE §3): the phone-home audit, hardened Vivaldi defaults, encrypted DNS. Then AppArmor.
4. Decisions still open ([CATCH_UP_2026-11-07.md](CATCH_UP_2026-11-07.md) §A): saved passwords from gnome-keyring to oo7;
   kdenlive or ghostwriter; Secure Boot with my own keys; how Super+S stops recording; frame generation's name.
5. The installer: shrink a partition to make room, partitions on more than one disk, the keyboard layout at the disk password.
6. The desktop: a Yggdrasil task manager, a global menu, accessibility (sticky keys, magnifier, on-screen keyboard, screen reader).

## Known issues

- Ctrl+Shift+Esc opens `btop` in a terminal; Yggdrasil has no task manager of its own yet.
- No global menu: apps keep their own menus.
- Secure Boot is not set up. Turn it off in the firmware before booting the stick.
- Installing beside another system needs free space or a partition to give up; the installer cannot shrink one yet.
