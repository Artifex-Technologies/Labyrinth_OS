# Before go-live: what to check

Written 2026-10-13 from everything that was built but could only be tried in the VM. "Tested" means seen
working in the VM; the items below are what the VM could not show (no audio, no Bluetooth or Wi-Fi, no
NVIDIA card, no physical keys, one screen).

## Confirmed on real hardware (the owner, 2026-10-13)

- The installer ran start to finish on the Dell OptiPlex with no problems; sleep and hibernate work there.
- Both screens came up on the main PC (Corsair One) from the live USB; the OptiPlex booted fine.
- Secure Boot is off on both machines, and both booted.
- Lock and unlock work on the OptiPlex; the shortcuts worked on both machines.
- Not done: unlocking the live USB (its password is not known), gaming (after go-live), the rest below.

## 1. Before you wipe Windows (blockers)

- [ ] **Back up what is on the machine** and keep Windows bootable (or a full image) until the new system
      has been used for a week.
- [ ] **A clean ISO build from scratch** (not the VM's hand-installed binaries): `final3.sh`, checksum, then
      a fresh install of it in the VM and every item in `docs/FIRST_BOOT.md`. Many things this session were
      tried by copying files into the VM; the ISO must contain them (the Fylgja code, `labyrinth-minos-icon`,
      the brand-pack data, the OSD override, the start-menu key, the icons).
- [x] **Boot the ISO live from USB on the real PC** (done, both screens up; the rest of this line still to look at) and look at: both monitors (the Acer and the DualUp),
      their layout and refresh rates, the NVIDIA driver loading (open kernel modules), Wayland session starts.
- [x] **The installer on real hardware** (done on the OptiPlex; check the disk encryption and the Windows entry on the main PC): partitioning (a spare disk first), disk encryption if offered,
      the bootloader (GRUB menu, the Windows entry, the UEFI entry), first boot, first login.
- [ ] **Disk encryption (LUKS)**: confirm it is offered and works; decide whether it is on by default.
- [x] **Suspend, resume, hibernate, shutdown, reboot, lock and unlock** (done on the OptiPlex) with a password (the lock screen
      was only seen being locked in the VM).
- [ ] **A way back**: a snapper snapshot exists after install, the GRUB snapshot menu works, and a rescue USB
      is ready.

## 2. Security

- [ ] **The package repo with `SigLevel = Optional TrustAll`** in `profile/pacman.conf`: find which repo it is
      and sign it, or remove it. Unsigned packages are the biggest hole I know of.
- [ ] **The installed system must not inherit the live user's passwordless sudo, autologin, or empty
      password.** Check `/etc/sudoers.d`, `/etc/sddm.conf.d`, `/etc/shadow` on a finished install.
- [ ] **Firewall** is on (ufw: deny incoming) after install; nothing listens on the network that should not
      (`ss -tulpn`): no SSH, no Ollama or Sarina server open to the LAN (Ollama binds to 127.0.0.1).
- [ ] **Secure Boot**: not set up; decide whether that is acceptable (it is for most Arch-based systems).
- [ ] **Sandboxing**: AppArmor or similar is not in place; Flatpak apps get what they ask for.
- [ ] **Updates**: how updates reach the machine (mirrors, who signs Labyrinth's own packages), and that a
      failed update can be rolled back.

## 3. Privacy (the stated goal)

- [ ] **Phone-home audit**: boot the installed system, leave it idle for ten minutes with a packet capture
      (`tcpdump` or `ss`/`nethogs`), list every host it talks to, and justify each one (mirrors, NTP,
      weather, Cloudflare for the speed test only when pressed, album art from the web when a song plays).
- [ ] **No telemetry** in anything shipped: Vivaldi (default browser) has options to switch off; Firefox
      is the backup. Decide the browser defaults and set the hardened ones.
- [ ] **DNS**: encrypted DNS and tracker blocking are not set up yet.
- [ ] **Heimdall**: the package is listed ("watches Sarina") but I have not verified what it does for
      data brokers; say what is and is not claimed in the README before anyone relies on it.
- [ ] The clipboard history is in memory only and skips password managers; check that on a real session
      with a real password manager.

## 4. The desktop on real hardware (never seen outside the VM)

- [x] **Keys** (the owner says the shortcuts worked): a bare Super tap opens the start menu; Super+E (Minos), Super+V (clipboard), Super+S (Sarina),
      Alt+Space / Alt+F2 (start menu search), Super+Return; the volume and brightness keys.
- [ ] **Fylgja with real audio**: the now-playing card and cover, play/pause/skip, the master volume (click
      and drag), the per-app mixer, choosing another output, Wi-Fi and Bluetooth toggles, Night Light actually
      tinting the screen, Dark mode from its tile.
- [ ] **Fylgja Stats**: the NVIDIA tile (clock, VRAM), the CPU temperature, the NVMe disk's name, the speed
      test on the real line (it reads low on gigabit).
- [ ] **Volume and brightness pop-up**: on a real key press, mute, and brightness on external monitors
      (probably the newer `org.kde.ScreenBrightness` service, not the one watched now); dock on a side.
- [ ] **Top bar and dock**: both monitors, hot-plugging, fractional scaling (the DualUp at 125%), the dock
      on each edge, windows stopping at the dock, full-screen games and video.
- [ ] **Lock screen and login**: unlock, the 12-hour clock (set it to follow the 24-hour setting), the
      Labyrinth logo that did not show on the login screen, two monitors.
- [ ] **Minos**: opening very large folders, drag and drop to and from other apps, Proton Drive, trash, and
      after an icon pack change (restart it).
- [ ] **Games and CAD**: Steam and a Proton game, Daedalus CAD, a large file, to see the machine stays
      smooth with the panel, Fylgja and Sarina's model server around (the model competes for VRAM).

## 5. Known gaps to decide on

- No system tray (StatusNotifier), global menu, or notification centre yet; KDE's own notifications show.
- Weather, the Wi-Fi/Bluetooth pages in Settings, workspaces profiles, the Notification Center, the
  screenshot tool, the system monitor, and Minos as the file chooser are not built.
- Sarina by voice (Super+S): the command line is a guess until her repository can be checked.
- Fylgja is built in; it is meant to become a separate add-on later.

## 6. Licences and names

- [ ] Licence files shipped for: WhiteSur (GPL-3), Klassy, Simple Icons (CC0; the logos
      remain trademarks of their owners), grub-evodevo (MIT), EdenExplorer (MIT).
- [ ] **Icons from macosicons.com** and other community icons: their licences are not clear. Do not ship
      them in a public ISO until that is settled; the Artifex icon pack is meant to replace them.
- [ ] The README says what the OS is built from (CachyOS, KDE) and does not claim to be, or look like, a
      product that is not ours in a way that could confuse.

## 7. Performance and size

- [ ] Idle memory and CPU of the panel and Fylgja (Fylgja reads things only while open).
- [ ] ISO size: (MacTahoe and the 1024 px app PNGs, the first things to trim, are gone: the icon pack is now Liki, in the Yggdrasil repository) the first things to
      cut (see `docs/ICONS.md`).
- [ ] Boot time, and time to a usable desktop.

## 8. Documentation

- [ ] README screenshots refreshed (they predate the notch, the new dock icons and the lock screen).
- [ ] `docs/STATUS.md` and `docs/FIRST_BOOT.md` brought up to date, with a "known issues" list.

## 9. Claude Code on this system

Claude Code installs per user with Anthropic's own installer and was run in the VM: `claude --version` works,
`claude -p` says "not logged in" (as it should before signing in), the sandbox's `bwrap` works. What it needs
and how it is provided is in [CLAUDE_CODE.md](CLAUDE_CODE.md).

- [ ] On the installed system: `labyrinth-claude-code`, then `claude` in a terminal, sign in through the
      browser, open a real project, and let it run a command, read and change a file, and use git.
