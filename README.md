# Labyrinth OS

## Screenshots

![The desktop: transparent top bar, the dock, and notification pop-ups at the bottom right](assets/screenshots/desktop.png)

*The desktop: a transparent top bar (the Labyrinth button, workspaces, the open app's name, network, Bluetooth, and one date/time pill), the dock, and a notification pop-up stack at the bottom right. The black tab at the top middle is Fylgja.*

| | |
| --- | --- |
| ![The start menu](assets/screenshots/start-menu.png) | ![The calendar](assets/screenshots/calendar.png) |
| *The start menu (Folders theme): search, favourites, apps by folder, and a power menu.* | *The calendar: scroll the wheel for other months.* |
| ![Minos, the file explorer](assets/screenshots/minos.png) | ![Notification Center pane](assets/screenshots/notification-center.png) |
| *Minos, the file explorer: tabs, a places sidebar, a details pane, search, cloud drives, and drag and drop.* | *The optional Notification Center pane: the calendar with your notifications under it, grouped by app.* |
| ![Settings, Appearance](assets/screenshots/settings.png) | ![Settings, Apps](assets/screenshots/settings-apps.png) |
| *Settings > Appearance: light or dark, accent colour, icon packs, dock, notifications.* | *Settings > Apps: usage, size, restart or close, remove, and a network switch for each app.* |

### Fylgja, the notch

Fylgja hangs from the black tab at the top middle of the screen. Rest the pointer on it (or use the wheel or a shortcut) and it drops down
with what is playing; open it fully and it has sections for the things you reach for most. It is drawn by Labyrinth's own panel, not by KDE.

| | | |
| --- | --- | --- |
| ![Fylgja: Stats](assets/screenshots/fylgja-stats.png) | ![Fylgja: Notifications](assets/screenshots/fylgja-notifications.png) | ![Fylgja: Wallpaper](assets/screenshots/fylgja-wallpaper.png) |
| *Stats: CPU, graphics card, memory, disk, network, the busiest apps, and an internet speed test.* | *Notifications: grouped by app, newest group first; open a group to see them all, click one to go to the app.* | *Wallpaper: pick one from the folder.* |

Its other sections are Controls (Wi-Fi, Bluetooth, sound, Night Light, dark mode, volume mixer, what is playing), Today (clock and date),
Clipboard (history kept in memory only) and Power. Super+S records a spoken question for Sarina and shows her answer on the same card. See
[docs/FYLGJA.md](docs/FYLGJA.md) and [docs/NOTIFICATIONS.md](docs/NOTIFICATIONS.md).

### Minos, the file explorer

Minos is Labyrinth's own file explorer (Rust and egui, modelled on EdenExplorer). Tabs, a places sidebar with favourites, storage and cloud
drives (Proton Drive connects from the sidebar), a details pane, search that can look inside subfolders, copy and move with a progress window,
inline rename, drag and drop between Minos and other apps, and the Trash. It is meant to become the system's open and save dialog.

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

- [DESKTOP.md](docs/DESKTOP.md): the Hyprland desktop, bar, what came from Omacosy
- [COMPONENTS.md](docs/COMPONENTS.md): optional software (Spotify, Obsidian, Steam, Proton ...)
- [WEATHER.md](docs/WEATHER.md): the multi-model weather engine
- [INSTALLER.md](docs/INSTALLER.md): Calamares install, disk layout, Secure Boot
- [PROTON.md](docs/PROTON.md): Proton (games), Proton Drive, Proton Pass
