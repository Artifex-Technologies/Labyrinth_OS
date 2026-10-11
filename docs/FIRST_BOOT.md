# Flashing the ISO and the first test (Dell OptiPlex 3080)

Written for: the owner, testing Labyrinth OS on a spare OptiPlex 3080 before touching the main PC.
The OptiPlex has two 256 GB drives (one M.2, one SATA), 8 GB RAM and Intel graphics.

## What you need

- The ISO: `C:\Users\grego\Downloads\Labyrinth-OS\labyrinth-2026.10.10-x86_64.iso` (about 10 GB;
  `SHA256SUMS` next to it).
- A USB stick of **16 GB or more**. Everything on it is erased.
- A DisplayPort or HDMI cable and a screen for the OptiPlex, a keyboard and a mouse.
- A network cable for the OptiPlex (Wi-Fi may not be present on a 3080).

## 1. Flash the USB stick (on Windows)

1. Download **Rufus** (rufus.ie) and open it.
2. Device: your USB stick. Boot selection: the ISO above.
3. Partition scheme: **GPT**, target system: **UEFI (non CSM)**.
4. Press START. If Rufus asks, choose **"Write in DD Image mode"**. Do not choose ISO mode.
5. Wait for READY (10 to 20 minutes for 10 GB), then eject the stick.

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
by itself: menu bar on top, a floating dock at the bottom, window buttons on the left. Try it first
without installing:

- Does the desktop appear on the screen at its full resolution and refresh rate? Is the network up?
- Open a window (the terminal icon in the dock). Its menus (File, Edit, View...) should appear in the
  top bar. Press F12 for the drop-down terminal. Tap the Windows (SUPER) key for the launcher.
- If you see "ERROR: no root file system image found" and a `[rootfs ~]#` prompt, power off and
  boot the stick again. This happened on the first boot of a fresh ISO in the VM and not on the
  second; if it happens every time on real hardware, tell me what the screen says.

## 4. Install (erases the target drive)

1. Start the installer: double-click **Install Labyrinth** on the desktop, or tap SUPER and type "install".
2. Follow the pages. On **Partitions**, pick the drive to erase in "Select storage device"
   (one of the two 256 GB drives; the other is left alone). Leave "Erase disk" selected.
3. Choose a name and a password; click Install. It takes about 10 minutes.
4. Restart when it says "All done", remove the USB stick, and log in.

The first login builds the desktop layout (a few seconds). To try the tiling desktop (Hyprland), open
the launcher and run "Switch to Tiling Desktop"; it logs you out and starts it. "Switch to macOS-style
Desktop" in its power menu (SUPER + ESCAPE) comes back.

## 5. What to check after the first real boot

- The GRUB menu appears for ~2 seconds, then the desktop login (SDDM, session "Hyprland").
- `systemctl is-system-running` says `running`; `systemctl --failed` lists nothing.
- `cat /proc/cmdline` contains `resume=UUID=...` and `resume_offset=...`; `swapon --show` lists
  `/swap/swapfile` (about 8 GB here) and zram.
- **Hibernate** (power button in the bar, then Hibernate; or `systemctl hibernate`): the machine
  powers off. Turn it on: it should come back to the same windows. This is the one thing the VM
  could not prove (the VM reset right after resuming), so please report exactly what happens.
- **Snapshots**: `sudo snapper list`; after rebooting, look for a snapshots submenu in the GRUB menu (grub-btrfs adds it once a snapshot exists; the first one appears after snapper's timer runs, or create one with `sudo snapper create`).
- Vivaldi opens (it is the default browser; Firefox is installed as the backup), Finder opens and
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
done once per monitor; anything you change later in System Settings is kept. To redo it:
`rm ~/.config/labyrinth/displays-kde.json` and log in again.

