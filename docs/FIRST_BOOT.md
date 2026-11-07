# Flashing the ISO and the first boot

How to write the ISO to a USB stick, try Labyrinth live, install it, and what to check after the first start. Updated
2026-11-07: Yggdrasil is the desktop, greetd the login screen, and Yggdrasil's installer the only installer.

## What you need

- The ISO (`labyrinth-DATE-x86_64.iso`, under 4 GB) and its `SHA256SUMS`.
- A USB stick of **8 GB or more**. Everything on it is erased.
- A screen, a keyboard and a mouse. A network cable helps: the parts chosen on the installer's Optional software page come from
  the internet at the first login.

## 1. Flash the USB stick (on Windows)

1. Download **Rufus** (rufus.ie) and open it.
2. Device: your USB stick. Boot selection: the ISO.
3. Partition scheme: **GPT**, target system: **UEFI (non CSM)**.
4. Press START. If Rufus asks, choose **"Write in DD Image mode"**. Do not choose ISO mode.
5. Wait for READY (about 5 to 15 minutes), then eject the stick.

Optional check: in PowerShell, `Get-FileHash <path to iso> -Algorithm SHA256` should match `SHA256SUMS`.

Afterwards Windows may call the stick unusable or write-protected. `scripts/windows/Reset-UsbDrive.cmd` fixes that (see the README).

## 2. Firmware (BIOS) settings

- **Boot mode: UEFI** (not Legacy or CSM). The installer needs UEFI.
- **Secure Boot: off.** Labyrinth does not set up Secure Boot yet.
- **Storage: AHCI.** Some PCs (Dell's often) ship with "RAID On", under which Linux cannot see an M.2 drive. Changing it stops an
  existing Windows on that machine from booting, so only change it on a disk you will erase.

## 3. Boot the stick and try it live

Pick the stick's UEFI entry in the firmware's boot menu (F12 on Dell, F11 or F8 on many others). The boot screen shows
Labyrinth's logo, then the Yggdrasil desktop comes up by itself: the live user is signed in without a password. Try it before
installing:

- Does every screen come up at its full resolution and refresh rate? Is the network up (top bar)?
- Tap the **Super** (Windows) key: the start menu opens. Super+Return opens a terminal (Ghostty), Super+E opens Minos.
- If you see "ERROR: no root file system image found" and a `[rootfs ~]#` prompt, power off and boot the stick again.

## 4. Install

1. Tap Super, type "install", and open **Install Labyrinth**.
2. The pages, in order:
   - **Welcome:** the language.
   - **Keyboard:** first the arrangement (QWERTY, QWERTZ, AZERTY, Dvorak, Colemak or another alphabet), then the layout. Try it in
     the box.
   - **Time zone:** search by place.
   - **Disk:** choose the disk, then erase all of it or choose partitions by hand (for example beside Windows, keeping its EFI
     partition). Encryption is on by default: type its password twice.
   - **Your account:** your name, the account name, the computer's name, the password twice.
   - **Browsers:** Vivaldi is on the medium; tick others (Firefox, Brave, LibreWolf ...) and pick the default.
   - **Optional software:** Sarina and Heimdall, and device software (Logitech, gaming mice, Razer, Corsair, Xbox controllers, HP
     printers). An entry is ticked already when such a device is plugged in.
   - **Summary:** check it, and tick the box that says the disk is erased.
3. **Installing** shows each step. When it says it is done, press **Restart now** and remove the stick.

What it does: a 1 GiB EFI partition and the rest for the system (or the partitions you chose), LUKS2 if encryption is on, btrfs
with `@`, `@home`, `@cache` and `@log`, the system copied from the stick, your account (an administrator with sudo; root stays
locked), GRUB with Labyrinth's menu, snapper, a swap file for hibernation, and the firewall on. Details:
[Yggdrasil's INSTALLER.md](https://github.com/Artifex-Technologies/Yggdrasil/blob/main/docs/INSTALLER.md).

## 5. The first start

- With encryption, GRUB asks for the disk password first. You type it once: the system opens the disk with a key kept inside it.
- The GRUB menu shows for about 2 seconds, then the boot screen, then Yggdrasil's login screen (greetd).
- At the first login, the parts ticked on the Optional software page that were not on the stick are installed, online. Notifications
  say what is happening, and the password box asks once. Without a network it tries again at the next login; Agora can install
  them at any time.

## 6. What to check

- `systemctl is-system-running` says `running`; `systemctl --failed` lists nothing.
- No live leftovers: `ls /etc/sudoers.d` has no `00-live-nopasswd`, `/etc/greetd` has no `live.toml`, and there is no `live` user.
- **Firewall:** `sudo ufw status` says active, incoming denied, outgoing allowed. Settings > Firewall shows the same.
- **Snapshots:** `sudo snapper list` shows "Fresh Labyrinth OS install". A snapshots submenu appears in GRUB once a snapshot exists.
- **Hibernate:** `swapon --show` lists `/swap/swapfile` and zram; `cat /proc/cmdline` has `resume=` and `resume_offset=`. Choose
  Hibernate in the power menu: the machine powers off, and on the next start it comes back to the same windows.
- Vivaldi (or the default you chose) opens web links, Minos previews files, sound and the network work, and wireless keyboards do
  not drop keys.

If anything fails, note what is on the screen (a photo works).

## 7. Sarina (if you chose her)

Open **Sarina** from the start menu. It starts the model server (Ollama) if it is not running, and if no model is installed it
suggests one that fits your memory and asks before downloading it (a few gigabytes). **Sarina Console** opens her web console in
the browser. From a terminal: `labyrinth-sarina`, `labyrinth-sarina gui`, `labyrinth-sarina model`. Super+S asks her a question by
voice (tap to start, tap again to send).

## 8. Monitors

Until you set them, every monitor runs at its fastest mode. Settings > Displays sets the resolution, refresh rate, scale, layout
and Night Light; it saves them in `~/.config/yggdrasil/monitors.lua`.

## 9. Coming from Windows

The Windows shortcuts work:

| Keys | Does |
|---|---|
| Super (tap) | Start menu |
| Super + E | Minos (files) |
| Super + I | Settings |
| Super + L | Lock |
| Super + V | Clipboard history |
| Super + Shift + S, Print | Screenshot (area, window or screen) |
| Ctrl + Shift + Esc | Running programs (`btop`) |
| Super + arrow keys | Snap the window (up maximizes, down minimizes) |
| Super + T | Tile or float the window |
| Alt + Tab, Alt + F4 | Switch windows, close a window |
| Super + F | Full screen |
| Super + N, Super + C | Notifications, calendar |
| Super + / | Every shortcut |

**Settings** (Super + I) is Yggdrasil's own: Wi-Fi & Network, Bluetooth, displays, sound, keyboard, printers, power, appearance,
apps, games, security, the firewall, restore points, users, date and time.

In the terminal, `cls` clears the screen and `ipconfig` / `ipconfig /all` print the network setup in the Windows layout
(`/release`, `/renew` and `/flushdns` work too). `ip a` shows the Linux view.
