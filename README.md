# Labyrinth OS

## Screenshots

![The desktop: transparent top bar, the dock, and notification pop-ups at the bottom right](assets/screenshots/desktop.png)

*The desktop: a transparent top bar (the Labyrinth button, workspaces, the open app's name, network, Bluetooth, and one date/time pill), the dock, and a notification pop-up stack at the bottom right. The black tab at the top middle is Fylgja.*

| | |
| --- | --- |
| ![The start menu](assets/screenshots/start-menu.png) | ![The calendar](assets/screenshots/calendar.png) |
| *The start menu (Folders theme): search, favourites, apps by folder, and a power menu.* | *The calendar: scroll the wheel for other months.* |
| ![Notification Center pane](assets/screenshots/notification-center.png) | ![Settings, Appearance](assets/screenshots/settings.png) |
| *The optional Notification Center pane: the calendar with your notifications under it, grouped by app.* | *Settings > Appearance: light or dark, accent colour, icon packs, dock, notifications, lock screen.* |

### Settings

Settings is Labyrinth's own app (Rust and egui), one page per area.

| | |
| --- | --- |
| ![Settings, Apps](assets/screenshots/settings-apps.png) | ![Settings, one app](assets/screenshots/settings-app-detail.png) |
| *Settings > Apps: usage, size, restart or close, remove, and a network switch for each app.* | *One app: its network access, startup, storage, and an optional sandbox.* |
| ![Settings, Security](assets/screenshots/settings-security.png) | ![Settings, Lock screen](assets/screenshots/settings-lock-screen.png) |
| *Settings > Security: Heimdall, the password log (login and sudo history) and the distress password.* | *Settings > Appearance > Lock screen: pick a design (used once Labyrinth's own lock screen is in service).* |

#### Distress password

A second password that quietly does something else when it is typed where your real one is asked for. The page opens only after you type your
account password, and nothing is saved until you press Save. See [docs/DISTRESS_PASSWORD.md](docs/DISTRESS_PASSWORD.md). (These pictures show
sample settings.)

| | |
| --- | --- |
| ![Distress password, opening the page](assets/screenshots/settings-distress-open.png) | ![Distress password, the settings](assets/screenshots/settings-distress.png) |
| *Opening the page asks for your account password.* | *Turn it on, set the password, and how many times it must be typed within how long.* |
| ![Distress password, the two sections](assets/screenshots/settings-distress-sections.png) | ![Distress password, saving the second section](assets/screenshots/settings-distress-save.png) |
| *Section 1 (no extra password to save) and Section 2 (asks for your password again).* | *Saving Section 2 shows a warning and asks for your password once more.* |

### The Marketplace

The Marketplace is the one place for apps: Labyrinth's reviewed catalog, the Arch repositories, the AUR and Flathub behind one search, with
installs, updates, history and source repositories you can add. See [docs/MARKETPLACE.md](docs/MARKETPLACE.md).

| | |
| --- | --- |
| ![Marketplace, Discover](assets/screenshots/marketplace.png) | ![Marketplace, Play](assets/screenshots/marketplace-play.png) |
| *Discover: featured apps and picks for creating, working and playing.* | *A category: emulation, game stores, games, music and video.* |
| ![Marketplace, an app](assets/screenshots/marketplace-app.png) | ![Marketplace, Updates](assets/screenshots/marketplace-updates.png) |
| *An app: screenshots, description, licence, source and where it comes from.* | *Updates: every update with its versions, and Update everything.* |
| ![Marketplace, search](assets/screenshots/marketplace-search.png) | |
| *Search across the catalog, repositories and Flathub.* | |

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

| | |
| --- | --- |
| ![Minos, the file explorer](assets/screenshots/minos.png) | ![Minos, a folder of pictures](assets/screenshots/minos-pictures.png) |
| *Tabs, a places sidebar, a details pane, search, cloud drives, and drag and drop.* | *A folder with its details pane: type, size and date for the selected item.* |

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
- [COMPONENTS.md](docs/COMPONENTS.md): optional software (Spotify, Obsidian, Steam, Proton Drive/Pass ...)
- [WEATHER.md](docs/WEATHER.md): the multi-model weather engine
- [INSTALLER.md](docs/INSTALLER.md): Calamares install, disk layout, Secure Boot
- [PROTON_GAMES.md](docs/PROTON_GAMES.md): Valve's Proton (Windows games in Steam)
- [PROTON_AG.md](docs/PROTON_AG.md): Proton AG's services, Proton Drive and Proton Pass
