# Flashing the ISO and the first test (Dell OptiPlex 3080)

Written for: the owner, testing Labyrinth OS on a spare OptiPlex 3080 before touching the main PC.
The OptiPlex has two 256 GB drives (one M.2, one SATA), 8 GB RAM and Intel graphics.

## What you need

- The ISO: `C:\Users\grego\Downloads\Labyrinth-OS\labyrinth-2026.10.13-x86_64.iso` (about 6 GB;
  `SHA256SUMS` next to it).
- A USB stick of **8 GB or more** (16 GB to be safe). Everything on it is erased.
- A DisplayPort or HDMI cable and a screen for the OptiPlex, a keyboard and a mouse.
- A network cable for the OptiPlex (Wi-Fi may not be present on a 3080).

## 1. Flash the USB stick (on Windows)

1. Download **Rufus** (rufus.ie) and open it.
2. Device: your USB stick. Boot selection: the ISO above.
3. Partition scheme: **GPT**, target system: **UEFI (non CSM)**.
4. Press START. If Rufus asks, choose **"Write in DD Image mode"**. Do not choose ISO mode.
5. Wait for READY (~5 to 15 minutes), then eject the stick.

Optional check: in PowerShell, `Get-FileHash <path to iso> -Algorithm SHA256` should match `SHA256SUMS`.

## 2. OptiPlex BIOS settings (press F2 at the Dell logo)

- **Storage / SATA Operation: AHCI** (Dell often ships "RAID On", under which Linux cannot see
  the M.2 drive). Changing it stops an existing Windows install on that machine from booting;
  that is fine if you are erasing it.
- **Secure Boot: Disabled** (Boot Configuration or Secure Boot page).
- **Boot mode: UEFI** (not Legacy).
- Apply and exit.

## 3. Boot the stick

Press F12 at the Dell logo, pick the stick's UEFI entry. The macOS-style desktop (KDE Plasma) comes up
by itself: menu bar on top, a floating dock at the bottom, window buttons on the right (minimize,
maximize, close, as on Windows). Try it first
without installing:

- Does the desktop appear on the screen at its full resolution and refresh rate? Is the network up?
- Open a window (the terminal icon in the dock). Its menus (File, Edit, View...) should appear in the
  top bar. Press F12 for the drop-down terminal (Ghostty, with tabs). Tap the Windows (SUPER) key for the launcher.
- If you see "ERROR: no root file system image found" and a `[rootfs ~]#` prompt, power off and
  boot the stick again. This happened on the first boot of a fresh ISO in the VM and not on the
  second; if it happens every time on real hardware, tell me what the screen says.

## 4. Install (erases the target drive)

1. Start the installer: double-click **Install Labyrinth** on the desktop, or tap SUPER and type "install".
2. Follow the pages. On **Partitions**, pick the drive to erase in "Select storage device"
   (one of the two 256 GB drives; the other is left alone). Leave "Erase disk" selected.
3. Choose a name and a password; click Install. It took 2 minutes in the VM; ~3 to 6 minutes from a
   USB stick, depending on the stick.
4. Restart when it says "All done", remove the USB stick, and log in.

The first login builds the desktop layout (a few seconds). To try the tiling desktop (Hyprland), open
the launcher and run "Switch to Tiling Desktop"; it logs you out and starts it. "Switch to macOS-style
Desktop" in its power menu (SUPER + ESCAPE) comes back.

## 5. What to check after the first real boot

- The GRUB menu appears for ~2 seconds, then the desktop login (SDDM, session "Plasma").
- `systemctl is-system-running` says `running`; `systemctl --failed` lists nothing.
- `cat /proc/cmdline` contains `resume=UUID=...` and `resume_offset=...`; `swapon --show` lists
  `/swap/swapfile` (about 8 GB here) and zram.
- **Hibernate** (power button in the bar, then Hibernate; or `systemctl hibernate`): the machine
  powers off. Turn it on: it should come back to the same windows. This is the one thing the VM
  could not prove (the VM reset right after resuming), so please report exactly what happens.
- **Firewall**: `sudo ufw status` says active, incoming denied, outgoing allowed (Settings >
  Firewall manages it).
- **Snapshots**: `sudo snapper list` shows "Fresh Labyrinth OS install" (kept for good); after rebooting, look for a snapshots submenu in the GRUB menu (grub-btrfs adds it once a snapshot exists; the first one appears after snapper's timer runs, or create one with `sudo snapper create`).
- Vivaldi opens (it is the default browser; Firefox is installed as the backup), Minos opens and
  previews files, sound and the network work, and wireless keyboards do not drop keys.

If anything fails, note what is on the screen (a photo works) and tell me.

## 6. Sarina (the local AI agent)

Open **Sarina** from the launcher (tap SUPER, type "sarina"). It starts the model server (Ollama) for
you if it is not running, and if no model is installed it suggests one that fits your memory and asks
before downloading it (a few gigabytes). **Sarina Console** opens her web console in the browser.
From a terminal: `labyrinth-sarina`, `labyrinth-sarina gui`, `labyrinth-sarina model`.

## 7. Monitors

On first login each monitor is set to its native shape at the highest refresh rate it offers at 60 Hz
or more. Your Acer ultrawide and the LG DualUp are recognised by name and get the layout from
`/usr/share/labyrinth/displays.json` (Acer 3440x1440 at 144 Hz, LG at 125% placed to its right). It is
done once per monitor; anything you change later in Settings is kept. To redo it:
`rm ~/.config/labyrinth/displays-kde.json` and log in again.

## 8. Coming from Windows

The Windows shortcuts work on both desktops:

| Keys | Does |
|---|---|
| Win + E | Minos (Explorer) |
| Win + R | Run box (type an app or command) |
| Win + I | Settings |
| Win + L | Lock |
| Win + V | Clipboard history |
| Win + Shift + S | Screenshot of a region |
| Ctrl + Shift + Esc | Task manager (System Monitor) |
| Win + arrow keys | Snap the window (macOS-style desktop) |
| Alt + Tab, Alt + F4 | Switch windows, close a window |
| Win + F | Full screen (hides the dock and top bar) |
| Win + Space | In full screen: bring up the dock and top bar, Esc hides them again (on the tiling desktop it opens the launcher) |

**Your DualUp is the Omarchy-style screen.** Windows dragged or opened there tile automatically
(no title bars, gaps), it has its own workspaces 1-9, and on it some keys switch to their Omarchy
meaning: Win + Arrows moves focus, Win + Shift + Arrows swaps tiles, Win + 1..9 switches workspace,
Win + W closes the window, Win + Space opens the launcher, Win + T floats a window. On the Acer they
keep the Windows meaning (snap, dock apps, overview). Win + Shift + Alt + Left/Right throws a window
to the other monitor. Turn it off with `labyrinth-tiling off`; DESKTOP.md has the full table.

**Settings** (Win + I, or the gear in the dock) is Labyrinth's own: Wi-Fi, Bluetooth, displays, sound,
appearance (dark/light, accent colour, wallpaper), power, firewall, date and time, apps, shortcuts.
KDE's System Settings is hidden; Plasma's "Configure..." links open the matching page here.

In the terminal, `cls` clears the screen and `ipconfig` / `ipconfig /all` print the network setup in
the Windows layout (`/release`, `/renew` and `/flushdns` work too). `ip a` shows the Linux view.
