# Status (updated 2026-11-15)

What Labyrinth OS is today and what is in progress. The older dated entries (the Windows and WSL builds,
Plasma, SDDM, Calamares) are in this file's git history: `git log -p -- docs/STATUS.md`.

## What it is today

- **Base:** CachyOS's `x86-64-v3` repositories, the `linux-cachyos` kernel with `linux-cachyos-lts` as the fallback, NVIDIA's
  open modules. Built with archiso from `profile/` ([ARCHITECTURE.md](ARCHITECTURE.md)).
- **Desktop:** Yggdrasil, in its own repository ([Artifex-Technologies/Yggdrasil](https://github.com/Artifex-Technologies/Yggdrasil)),
  as a session on Hyprland 0.57. Top bar, dock, Fylgja (the notch), Settings, Agora (the app store), Minos (files), the lock
  screen, notifications, the screenshot tool, the clipboard history, the colour picker, the video player, spell check and the
  Task Manager (Ctrl+Shift+Esc; since 2026-11-15 with GPU columns, a page for every graphics card and the AI models Ollama has
  loaded). Small tools (Task Manager, Settings, calculator, the password box) float on tiling screens. Yggdrasil's own
  compositor is not the desktop yet: its hardware backend (DRM/KMS) reached its first milestone on 2026-11-15 in a virtual
  machine; the laptop tests are next. No KDE:
  Plasma, KWin and SDDM left on 2026-10-24; the old Omarchy-style tiling desktop (waybar, mako, fuzzel and the rest) on 2026-11-05.
- **Login:** greetd with Yggdrasil's greeter. On the live medium greetd signs the live user in by itself (`/etc/greetd/live.toml`,
  which the install removes).
- **Installer:** Yggdrasil's installer window (`yggdrasil-installer`) and its install program (`yggdrasil-install-system`; Labyrinth's
  own `labyrinth-install-system` until 2026-11-07, moved to Yggdrasil so LykanOS installs the same way). Labyrinth's settings for it
  (clean-up, last checks, boot menu theme, kernel options) are in `labyrinth-installer.conf`.
  Since 2026-11-14 its first step after Welcome is the network: Wi-Fi, Ethernet or a USB adapter, hidden networks, each
  adapter's MAC address, networks that turn up later, or skip; the install's downloads are fetched while the rest is asked.
  Erase a disk or choose partitions by hand (beside Windows, keeping its EFI partition); LUKS2 encryption on by default; btrfs with
  snapper and grub-btrfs; GRUB with Labyrinth's theme. Calamares was removed on 2026-11-07 (df5b5a2); there is no fallback installer.
- **Software:** the ISO stays under 4 GB. Vivaldi is the one browser on it. The installer's Browsers page adds others; its Optional
  software page offers Sarina, Heimdall and device software, which install at the first login, online. Agora has the rest
  ([COMPONENTS.md](COMPONENTS.md)).
- **Updates:** Labyrinth's signed package repository (GitHub release "packages", [PACKAGE_REPO.md](PACKAGE_REPO.md)) keeps an
  installed system current from any ISO. Since 2026-11-07 the ISO build's own packages are signed and checked too.
- **Security:** Heimdall, Limbo, the hardening defaults, the firewall (ufw, incoming blocked), the distress password, a network
  switch per app. Sarina is optional. How each part works: Heimdall's and Limbo's own documentation.
- **The privacy pack (2026-11-15):** the downloads guard, Remove hidden info in Minos, viewer apps on "ask" before they first go
  online, and look-alike name warnings in Agora and Settings > Apps, each on by default with its own switch in Settings >
  Security > Privacy ([PRIVACY_PACK.md](PRIVACY_PACK.md)).
- **Location sounds (2026-11-15):** the private microphone removes aircraft tones and mains hum, which can place or date a
  recording, from everything the microphone records (on by default; Settings > Security > Privacy), and Minos scrubs files.
  It shows up as "Microphone (private)". On every install (`heimdall-private-mic-git`, in the base system), with or without
  Heimdall; see [LOCATION_SOUNDS.md](LOCATION_SOUNDS.md).
- **Rust:** Labyrinth's and Yggdrasil's own tools are Rust; the Python and shell copies kept for a rollback were removed on
  2026-11-15 ([RUST_PORT_REMAINING.md](RUST_PORT_REMAINING.md)). Limbo is Rust only (its Go is gone). Heimdall's Rust port is
  done but not switched on yet. What Sarina installs is all Rust: her core, her ML/voice helper and `sarina-tone` since 2026-11-18, her
  voice service since 2026-11-17; her Go build, Python helper and Python voice service are gone.

**Seen working:** the ISO builds and boots (last built 2026-11-12, Labyrinth OS 1.0.0); the installer completes on real hardware,
also beside Windows and encrypted, and on the ThinkPad X1 Yoga from that ISO ([HARDWARE.md](HARDWARE.md)); the desktop runs on my
main PC every day. `scripts/release-check.sh` runs every check that needs no root
([RELEASE_CHECKS.md](RELEASE_CHECKS.md)); GitHub runs its quick part on every push, and it passes.

## In progress

- The laptop (ThinkPad X1 Yoga Gen 8): what works so far is in [HARDWARE.md](HARDWARE.md); the rest is being checked.
- Yggdrasil's own compositor on real screens: the hardware backend's next milestones, the laptop first.
- Sarina's training: her eval runs and their reports (kept with my private plans).
- Frame generation for games: Kairos, Labyrinth's own Vulkan layer (`kairos-git`, a preview; named 2026-11-07, it was
  `labyrinth-framegen`; the old package is replaced on update and its launcher name still works), with
  lsfg-vk as the stopgap. Settings > Games chooses the games.
- The GPU unlock ([GPU_UNLOCK.md](GPU_UNLOCK.md)): Phases 0 and 1 are done. Phase 2 waits on my go, four BIOS readings and a game
  to measure with.
- LykanOS (formerly Chymaera OS): its repository was renamed twice on 2026-11-07
  ([Artifex-Technologies/LykanOS](https://github.com/Artifex-Technologies/LykanOS)) and it is moving to Yggdrasil.
- Settings > Wi-Fi & Network: Heimdall's list of devices on the network and its Wi-Fi checks (through the labyrinth-security
  integration).

## Known limitations

- No global menu: apps keep their own menus.
- Secure Boot is not set up. Turn it off in the firmware before booting the stick.
- Installing beside another system needs free space or a partition to give up; the installer cannot shrink one yet.
