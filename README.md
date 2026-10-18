# Labyrinth OS

**Labyrinth OS** is the operating system (CachyOS-based, for one specific machine, with the Artifex ecosystem). **Yggdrasil** is its desktop environment: the
panel and dock, Fylgja, Settings, the Marketplace, Minos, the lock screen, the login screen, the idle manager, the polkit and portal pieces and, in progress, the
compositor. Yggdrasil is built to be reused: [Chymaera OS](https://github.com/ChymaeraLabs) will use it too. See [docs/YGGDRASIL.md](docs/YGGDRASIL.md).

## Screenshots

![The desktop: transparent top bar, the dock, and notification pop-ups at the bottom right](assets/screenshots/desktop.png)

*The desktop: a transparent top bar (the Labyrinth button, workspaces, the open app's name, network, Bluetooth, and one date/time pill), the dock, and a notification pop-up stack at the bottom right. The black tab at the top middle is Fylgja.*

### Yggdrasil, the desktop

The desktop (panel and dock, Fylgja, Settings, the Marketplace, Minos, the lock and login screens, the idle manager and the compositor) is developed in its own
repository, [Yggdrasil](https://github.com/Artifex-Technologies/Yggdrasil) (private while it is being fine-tuned), with its own README, screenshots and docs.
Labyrinth builds its packages from a checkout next to this repository (`scripts/build-local-packages.sh`). See [docs/YGGDRASIL.md](docs/YGGDRASIL.md).

### Distress password

A second password that quietly does something else when it is typed where your real one is asked for. The page opens only after you type your
account password, and nothing is saved until you press Save. See [docs/DISTRESS_PASSWORD.md](docs/DISTRESS_PASSWORD.md). (These pictures show
sample settings.)

| | |
| --- | --- |
| ![Distress password, opening the page](assets/screenshots/settings-distress-open.png) | ![Distress password, the settings](assets/screenshots/settings-distress.png) |
| *Opening the page asks for your account password.* | *Turn it on, set the password, and how many times it must be typed within how long.* |
| ![Distress password, the two sections](assets/screenshots/settings-distress-sections.png) | ![Distress password, saving the second section](assets/screenshots/settings-distress-warning.png) |
| *Section 1 (no extra password to save) and Section 2 (asks for your password again).* | *Saving Section 2 shows a warning and asks for your password once more.* |


### Installer and boot menu

| | |
| --- | --- |
| ![Installer welcome](assets/screenshots/installer-welcome.png) | ![Installer, your details](assets/screenshots/installer-users.png) |
| *The installer: the same dark look as the apps; the computer's name is asked here.* | *Name, login and computer name.* |

![The boot menu](assets/screenshots/boot-menu.png)

*The boot menu: Labyrinth's emblem, the UEFI firmware setting, and any other system found on the disks (Windows, another Linux). Theme: [grub-evodevo](https://github.com/ftonneau/grub-evodevo), MIT.*

(The installer and boot menu pictures are from a virtual machine and may look a little different from the current build.)

### Security: Heimdall, Limbo and Sarina

Heimdall watches the system (which program connects where, changed system files, the local agent's sessions) and raises alerts, which show up as
notifications; Limbo runs untrusted things in disposable virtual machines; Sarina is the local assistant. See [docs/SECURITY_ROADMAP.md](docs/SECURITY_ROADMAP.md).

A daily-driver Linux for one specific machine (for now), meant to replace Windows on it:

| Part | Hardware |
| --- | --- |
| CPU | Intel i9-9900K (Coffee Lake, AVX2, **no AVX-512**, so CachyOS `x86-64-v3`) |
| RAM | 64 GB |
| GPU | NVIDIA RTX 2080 Ti (Turing TU102, supported by the open kernel modules) |
| Disk | 1 TB |

Use cases: everyday desktop, music / photo / video work, CAD (Daedalus_CAD), gaming.

Labyrinth is a **distribution profile, not a kernel rewrite**. It does not
reimplement Linux; it assembles three upstreams plus the Artifex ecosystem:

| Upstream | Role in Labyrinth |
| --- | --- |
| [CachyOS](https://github.com/CachyOS/linux-cachyos) | Base: optimized `x86-64-v3` repos, `linux-cachyos` kernel (BORE/EEVDF, LTO), NVIDIA modules, gaming stack |
| [Omarchy](https://github.com/omacom/omarchy) | Desktop model: Hyprland, keyboard-first shortcuts, fast boot, AI integration |
| [EdenExplorer](https://github.com/mtucciarone/EdenExplorer) (MIT, Rust/egui) | Model for `minos`, the file explorer |
| Artifex ecosystem | Sarina, Limbo, Heimdall, Daedalus_CAD, Argus_App; shared with Chymaera_OS |

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): decisions and rationale
- [docs/DRIVER_LAB.md](docs/DRIVER_LAB.md): the Sarina + Limbo driver idea
- [docs/STATUS.md](docs/STATUS.md): what is built and what is not

## Status

Working: the ISO builds and boots, the installer completes, and the desktop (top bar, dock, start menu, Fylgja, notifications, Minos, Settings) runs on the
author's machine every day. Not finished: it is built for one machine first, and several parts are young. [docs/STATUS.md](docs/STATUS.md) says what is built and what is not.

## Documents

- [DESKTOP.md](https://github.com/Artifex-Technologies/Yggdrasil/blob/main/docs/DESKTOP.md): the Hyprland desktop, bar, what came from Omacosy
- [COMPONENTS.md](docs/COMPONENTS.md): optional software (Spotify, Obsidian, Steam, Proton Drive/Pass ...)
- [WEATHER.md](https://github.com/Artifex-Technologies/Yggdrasil/blob/main/docs/WEATHER.md): the multi-model weather engine
- [INSTALLER.md](docs/INSTALLER.md): Calamares install, disk layout, Secure Boot
- [PROTON_GAMES.md](docs/PROTON_GAMES.md): Valve's Proton (Windows games in Steam)
- [PROTON_AG.md](docs/PROTON_AG.md): Proton AG's services, Proton Drive and Proton Pass
