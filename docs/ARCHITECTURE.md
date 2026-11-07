# Architecture

## 1. Base: CachyOS, not Arch + patches

- Repos: `cachyos-v3`, `cachyos-core-v3`, `cachyos-extra-v3`. The 9900K has AVX2
  but not AVX-512, so `-v4` would not run. Do not use `znver4`.
- Kernel: `linux-cachyos` (EEVDF, Clang ThinLTO) by default and `linux-cachyos-lts`
  always installed as the **bootable fallback**. (`linux-cachyos-bore` was dropped
  2026-10-11: a third kernel meant a third initramfs and DKMS build on every install and
  update; `sudo pacman -S linux-cachyos-bore linux-cachyos-bore-headers` adds it back.)
- Do not hand-roll a kernel. Rewriting Linux is out of scope; CachyOS already
  tracks and patches it.
- Build system: archiso profile with CachyOS repos in `profile/pacman.conf`. Labyrinth's own
  packages come from a local `[labyrinth]` repository, signed with the Labyrinth key and checked
  (`SigLevel = Required`; [PACKAGE_REPO.md](PACKAGE_REPO.md)).
- Installer: Yggdrasil's installer window (`yggdrasil-installer`) with Labyrinth's install
  program (`labyrinth-install-system`), which copies the live image onto the disk. Calamares, the
  first plan ([INSTALLER.md](INSTALLER.md), kept for its reasoning), was removed on 2026-11-07.

## 2. GPU (RTX 2080 Ti)

- Default: `linux-cachyos-nvidia-open` (prebuilt, matches the kernel, no DKMS
  rebuild per kernel bump). Turing is supported by the open modules.
- Candidates the Driver Lab may compare: proprietary `nvidia` / `nvidia-dkms`,
  `nvidia-open-dkms`, and nouveau + NVK (Mesa). Wayland: `nvidia_drm.modeset=1`,
  `nvidia_drm.fbdev=1`.
- The i9-9900K has an Intel UHD 630 iGPU. Keep it enabled: it is the display
  fallback if the NVIDIA stack breaks, and it enables passthrough testing of
  the 2080 Ti (see DRIVER_LAB.md).
- Gaming: Steam, Proton-CachyOS, gamemode, mangohud, lib32 NVIDIA/Vulkan.

## 3. Desktop: Yggdrasil

Yggdrasil, our own desktop, in its own repository
([Artifex-Technologies/Yggdrasil](https://github.com/Artifex-Technologies/Yggdrasil)), runs as a
session on Hyprland (0.57 today) until its own compositor is ready. Login is greetd with
Yggdrasil's greeter. KDE (Plasma, KWin, SDDM) left on 2026-10-24, and the Omarchy-style tiling
desktop on 2026-11-05; Omarchy remains a source of ideas (keyboard-first shortcuts, plugins rebuilt
for Yggdrasil). Yggdrasil is shared with Niflheim OS (formerly Chymaera OS), which is moving to it.
What Labyrinth adds to the desktop (Heimdall, Sarina, the installer's steps) plugs in through
integration files in `/usr/share/yggdrasil/integrations`.

Speed targets (to be measured, not promised): GRUB with a 2 s timeout,
a `systemd-analyze` budget tracked per release, no unneeded boot services,
zram, Btrfs (zstd; `@`, `@home`, `@cache`, `@log`) with snapper and grub-btrfs
snapshot entries (GRUB, not Limine; reasons in INSTALLER.md) so a bad update rolls
back from the boot menu.

Creative / CAD / gaming layer, all optional ([COMPONENTS.md](COMPONENTS.md)): the `creative`
component (Darktable, OBS Studio); Krita, GIMP, Ardour and more one by one in Agora; Steam +
Proton; Daedalus_CAD (native Rust).

## 4. File explorer: `minos`

EdenExplorer is Windows-only (NT file APIs, Recycle Bin, shell context menu), so
it cannot be ported as-is. Its **feature set and UX** are the spec. Rust + egui
like the original (MIT; attribution goes in THIRD_PARTY_NOTICES). Linux
substitutions:

| Eden (Windows) | Labyrinth (Linux) |
| --- | --- |
| NT-level directory access | `std::fs` / `getdents64`, `inotify` for live updates |
| Recycle Bin | freedesktop Trash spec |
| Raw/unmounted drives, phones | `udisks2` over D-Bus; MTP via gvfs/libmtp |
| Shell context menu | `.desktop` actions, "Open with" via MIME database |
| Env-var address bar | `$VAR` / `~` expansion |

Keep: tabs + pinning + session restore, saved history, split pane, media gallery,
tags/folder colors, multi-column sort, favorites sidebar, real-time index search.
A Sarina hook ("organize these photos", "find duplicate takes") is a later phase.
`packages/minos` currently holds only the GUI-independent core
(tabs, history, address expansion).

## 5. Artifex apps

| App | Packaging | Notes |
| --- | --- | --- |
| Sarina | `sarina-git` PKGBUILD | Local model on the 2080 Ti (11 GB VRAM) competes with games for VRAM: needs a policy |
| Limbo | PKGBUILD, needs libvirt/QEMU/KVM | Linux guests verified; **Windows guests never run** |
| Heimdall | PKGBUILD | Watches Sarina |
| Daedalus_CAD | cargo build, wgpu/Vulkan | Needs working Vulkan on NVIDIA |
| Argus_App | Qt 6 build, udev rules (`setup_rules.sh`) | Flipper Zero console |

BlackArch is deliberately **not** included; this is a daily driver.

## 6. Replacing Windows: practical constraints

- Back up first; a full-disk install wipes Windows. Windows could live on in a
  Limbo guest (license-gated) for Windows-only apps.
- Some anti-cheat games (EAC/BattlEye configurations) do not run on Linux. Check
  ProtonDB for your library before committing.
- Adobe apps, Resolve Studio features and some DAW plugins are the usual pain
  points. List what you depend on and each one gets mapped to a replacement.
- Secure Boot: needs `sbctl` key enrollment, or disable it.

## 7. Obsidian

Installed from the official Arch `extra` repo (`obsidian`, repackaging upstream's
release; depends on `electron43`), so nothing is rebuilt or vendored here. The app
is proprietary (`LicenseRef-Obsidian`); the `github.com/obsidianmd` repos are
releases, docs, plugin API and tooling, not the app's source. Your account,
Sync and plugins work as on other platforms. Vaults are plain folders, so a vault
can live inside a linked Proton Drive folder and be checked with
`minos-cloud verify`; avoid running Obsidian Sync and Proton Drive on the
same vault at once.
