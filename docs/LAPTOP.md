# Laptop plan: ThinkPad X1 Yoga Gen 8

Written 2026-11-12 for the Claude session that runs on the laptop. Read this first, then work down the checklists. Tick items
here as they are done (`- [x]`), and add what you find. One file, so the session on my desktop PC and the one on the laptop see the
same plan.

## The device

- ThinkPad X1 Yoga Gen 8: 13th-gen Intel (Iris Xe), 32 GB, 2-in-1 with touchscreen and pen, TrackPoint, fingerprint reader,
  Intel AX211 Wi-Fi.
- Bought as the LykanOS machine (the security edition, not for gaming); also where I test Labyrinth on a laptop and run Icarus
  in the truck. For now it runs **Labyrinth OS 1.0**, installed from `labyrinth-1.0.0-2026.11.12-x86_64.iso` on 2026-11-12.
- BIOS: Secure Boot **off** (our builds are not signed yet), Intel AMT **disabled**.

Never write the laptop's serial numbers, MAC addresses, host name, network names or anything personal into a repository. These
repositories are public.

## How this laptop gets updates

- Labyrinth's own packages and Yggdrasil come from the signed `[labyrinth]` repo (GitHub release `packages`), published every day
  at 05:30 from my desktop PC. After that, `sudo pacman -Syu` on the laptop brings in that day's builds.
- So a fix pushed to a repository reaches the laptop the morning after, not at once. For something urgent, build the one package on
  the laptop (`makepkg` in its `packages/pkgbuilds/<name>` folder) and install it with `pacman -U`.
- Before pushing: `git pull --rebase`. Sessions on my desktop PC push to Yggdrasil and Labyrinth_OS all day. Stage files by path,
  never `git add -A`.

## Already done (2026-11-12)

From this laptop's own session (Yggdrasil):

- [x] Battery beside the clock (drawn like a Mac's, "72%"), power mode over D-Bus, charge limit (`c1d06b7`, `8d428b1`, `f3ac236`)
- [x] Settings > Power: fans and temperature as the firmware runs them (`0d23a97`)
- [x] TrackPoint scrolls with the middle button held (`572bb64`); touchpads scroll naturally (`90f08d9`)
- [x] The pen and touchscreen draw on the laptop's own screen (`3d21b74`)
- [x] Pull the notification panel (Fylgja) down from the notch with a finger (`453faa5`)
- [x] Time zone: searchable list, Windows-style names, the clock follows a change, "Set time zone automatically" (`b0d2b7a`,
      `ac8de3d`, `99c0371`, `431113c`)
- [x] Dock: drag pinned apps; app search finds keywords; start menu shows the name and power choices (`3b3a765`, `63edff8`)
- [x] Window buttons (minimize, maximize, close) on every window by default (`b7dcc14`); Weather °F before a place is set
      (`4c417ee`)
- [x] Labyrinth: Spotify and Obsidian left the ISO, Agora installs them; Agora pinned in the dock (`71c9e26`, `ff94fc5`)

From the desktop PC's session, for the laptop:

- [x] Settings: Enter joins a Wi-Fi network; the Wi-Fi password no longer goes on a command line (`726685a`)
- [x] Installer: the live session's saved Wi-Fi goes to the installed system (`f98c8bc`). This laptop was installed before that
      fix, which is why its Wi-Fi had to be joined again.
- [x] Settings > Peripherals: Mouse and Keyboard pages (pointer speed, repeat, layout, Razer lights) (`8db2e4a`)
- [x] Keyboard and mouse lights off while dimmed, fixed for good (`840fbc7`)
- [x] Laptop hardware set (2026-11-12): tablet mode and screen rotation with the touchscreen and pen turned along, the on-screen
      keyboard in tablet mode, Settings > Displays > Tablet mode (rotation lock), Settings > Users > Fingerprint (enrol; sudo
      with a finger, off until I turn it on), the installer's "This laptop" components (fingerprint, thermald, on-screen
      keyboard), the laptop's own screen at automatic scale. Yggdrasil `e2807e8`, `34817e8`, `934c24e`; Labyrinth `7426158`.
      Reference: [HARDWARE.md](HARDWARE.md). Reaches the laptop with the next day's `[labyrinth]` packages (yggdrasil-settings,
      yggdrasil-session, yggdrasil-installer); the components come from Agora.

## In progress on the desktop PC (do not start these here)

- **Location without Wi-Fi.** I don't want nearby Wi-Fi networks or their hardware IDs used to find where the computer is. The
  automatic time zone and Weather's "Automatic" place are moving to IP-only, and they pause while a VPN is on. Until that update
  arrives, leave "Set time zone automatically" **off**.
  2026-11-13: it had been switched on here the night before (it asked beaconDB once, at the first check); now off
  (`yggdrasil-autozone.timer` disabled). The time zone stays as set, by hand.
- **Laptop hardware set:** landed (see "Already done"); test it here (below). Still open on the desktop PC: fingerprint on the
  lock screen and in password boxes (the plan is in Yggdrasil's docs/DESKTOP.md), rotation on Yggdrasil's own compositor, and
  an on-screen keyboard you can open on demand.
- **Review of the commits above.** Another session reviews this laptop session's commits for security and for anything that breaks
  on a desktop without a battery. If it changes something you wrote, pull before you edit those files again.

## To check on the laptop (needs the hardware)

Write what you find under each item, and what works without changes.

Basics:
- [ ] Suspend and resume (lid close, power key, idle): comes back with Wi-Fi, sound and touch. Check `cat /sys/power/mem_sleep`
      (s2idle is expected) and battery drain overnight while suspended.
      2026-11-13: `mem_sleep` is `[s2idle]`, as expected. Resume and overnight drain still to try.
- [ ] Lid closed with an external screen attached: stays awake and uses only the external screen.
- [x] Brightness keys, volume keys, mic-mute key and its light, the F-keys' other functions (Fn lock)
      2026-11-13: all working. F1-F3 volume, F4 mic mute (its light follows), F5/F6 brightness with the pop-up: they did
      nothing at first because brightnessctl is not on a Labyrinth install; yggdrasil-mediakey now sets the backlight through
      logind when brightnessctl is missing (Yggdrasil 03db61e). F7 Settings > Displays, F8 airplane mode, F9 notifications, F12
      (the star key; ThinkPads send KEY_BOOKMARKS, XF86Favorites in the keymap) whatever Settings > Peripherals > Keyboard > Star
      key says, Sarina by default (219b6d2, 77c0f4c). F10/F11 (phone) left to the app in front. Fn+Esc is Fn lock. No ePrivacy
      screen on this unit (thinkpad_acpi lcdshadow: not supported).
- [ ] Sound: speakers and headphone jack (sof-firmware and alsa-ucm-conf are on the ISO); the microphones
- [ ] Camera: check whether it is a USB camera (`lsusb`) or a MIPI camera on Intel IPU6 (`lspci`; it would need the IPU6 driver
      and libcamera). Write down which, and don't install anything for it yet.
      2026-11-13: a **USB camera** (Chicony 04f2:b751, "Integrated Camera"), on the kernel's uvcvideo driver: /dev/video0-3
      exist. No IPU6, nothing to install.
- [ ] External screens over USB-C / Thunderbolt and HDMI: plug in and out while running; the dock and windows follow
- [ ] Thunderbolt devices are authorised (bolt is not on the ISO; check whether anything is needed)
      2026-11-13: two Thunderbolt domains, security level **`user`** (the firmware's default). At that level a Thunderbolt device
      (a dock, an eGPU) is not connected until something authorises it, and without bolt nothing does: expect a Thunderbolt dock
      to stay dark (USB-C displays and USB devices that are not Thunderbolt are unaffected). Not tried with a device yet; bolt is a
      package to ask about, or the BIOS level could be lowered (a BIOS change: ask).
- [ ] Battery: charge limit really stops charging at the set level; power modes change the fans and the battery life
- [ ] VS Code's workspace-trust dialog (and its other pop-ups) fits the screen at my scale, centred, with nothing to drag
      around (Yggdrasil `d058852`: no floating window opens bigger than its screen). Also check a window kept from before a
      scale change in Settings > Displays comes back no bigger than the screen.

2-in-1:
- [ ] Pen: pressure, the side buttons, the eraser end, palm rejection while writing
- [ ] Touch: tap, scroll, pinch in apps; touch on the right screen with an external one attached
- [ ] Rotation and tablet mode (Yggdrasil `934c24e`): after `sudo pacman -Syu`, log out and in (the watcher starts with the
      session: `/usr/lib/yggdrasil/tablet json` should say `"accelerometer":true,"tablet_switch":true`). Fold into tablet mode:
      the screen turns with the laptop, and in all four directions a finger and the pen land where they touch (if the screen turns
      the wrong way, or touch is mirrored, write which reading did it: `left-up`, `bottom-up`, `right-up`). Open it again: upright.
      Settings > Displays > Tablet mode > Lock rotation keeps the screen as it is. If nothing happens on folding, write what
      `hyprctl devices` lists under switches (the binds are named after them) and whether `/usr/lib/yggdrasil/tablet mode on`
      typed by hand turns it.
      2026-11-13: the packages arrived (yggdrasil-settings 1.0.0-32, -session 1.0.0-25) and `tablet json` says
      `"accelerometer":true,"tablet_switch":true`. Folding already turned the keyboard off (the firmware does that by itself), but
      the screen did not turn: the session had started at 09:08:00 and the update came at 09:08:52, so the watcher was not running
      (no `tablet` process). To try again after logging out and in.
- [ ] On-screen keyboard: add "On-screen keyboard" from Agora (wvkbd, from the AUR; ask me first). Folded with no keyboard plugged
      in it shows; plugging in a USB or Bluetooth keyboard hides it; opening the laptop hides it.
- [ ] Screen scale: the laptop's own screen now picks its scale automatically (Yggdrasil `34817e8`), as long as Settings >
      Displays has not set one. Check the scale it picked (`hyprctl monitors`), that text is a comfortable size, and that an external
      monitor plugged in stays at its own scale. If Settings > Displays already wrote a scale for eDP-1 before, that one is kept.
      2026-11-13: after logging out and in, Hyprland's automatic scale picked **150 %** for the 13.3-inch 1920x1200 panel
      (about 170 px per inch; Windows recommends 150 % too). Too big for me: I set **125 %** in Settings > Displays, which wrote
      eDP-1 to monitors.lua, and that is kept at every start, as designed. If 125 % should be the default for panels like this,
      the automatic rule would need its own steps rather than Hyprland's.

Security (this is the LykanOS machine):
- [ ] Disk encryption: was it chosen in the installer? If not, write it down. Reinstalling with encryption comes with LykanOS.
      2026-11-13: **not encrypted**. nvme0n1p2 is plain btrfs (no LUKS), with the EFI partition beside it; swap is zram.
- [ ] Heimdall is running (`systemctl status heimdall`), and its alerts make sense on a laptop moving between networks
      2026-11-13: running. Two false alarms on my phone's hotspot ("needs care"): "1 other device heard on this network" while
      the laptop was the only client (the device heard is the phone itself, the hotspot); and "the router's hardware address
      is different from last time": an iPhone's hotspot changes it when it restarts. `heimdall-wifi-guard trust` remembers the
      router's address, so it would keep tripping. Wanted: count the gateway out of "other devices", and a way to trust a phone
      hotspot whose address changes. Its Wi-Fi verdict shows an OWE-TM network by its hidden `_owetm_` name.
      Also seen: labyrinth-appnet-ask failed every 5 s from boot (no /etc/labyrinth; 1,933 restarts): fixed in Labyrinth
      7744b9b (ConfigurationDirectory), and /etc/labyrinth made by hand here meanwhile.
- [ ] The firewall (ufw) is on; Wi-Fi checks on public networks (Settings > Network)
      2026-11-13, a shop's guest network (Enhanced Open in transition mode, OWE-TM, with a sign-in page): Settings showed a lock
      and wanted a password (fixed: Yggdrasil 8c7c6e5). Joined, the laptop correctly took the encrypted hidden half, whose real
      name is `_owetm_` + part of the name + digits; **Heimdall's verdict shows that raw name** (to show the friendly one).
      Before the terms are accepted the network answers every https site with its own certificate: Heimdall called it
      critical (right) and every page said "not private". The panel's "Sign in to this Wi-Fi network" notification (Yggdrasil
      8c7c6e5, 5c983f5) **did not appear**: NetworkManager logged no portal state, so its check may not see this kind of portal.
      To do on the next such network: `nmcli networking connectivity check` while joined, and a manual "Open sign-in page"
      button in Settings > Wi-Fi for a network whose portal is not detected. The hotspot was used instead.
      2026-11-13: ufw is on (enabled, incoming dropped), and nothing listens on the network but Vivaldi's local-network discovery.
      No passwordless sudo, no live user and no live login were left over from the installer. Public-network checks still to do.
- [ ] Fingerprint reader: which one it is (`lsusb`: Synaptics 06cb or Goodix 27c6), and whether its id is in the installer's list
      (`installer-extras.json`, the readers libfprint supports). With my OK: add "Fingerprint reader" from Agora, enrol a finger in
      Settings > Users > Fingerprint, then (with my OK again, it changes /etc/pam.d/sudo) turn on "Use for sudo" and try
      `sudo -k; sudo true` with the finger, then with Ctrl+C and the password. Turning it off must leave /etc/pam.d/sudo as it was.
      2026-11-13: **Synaptics 06cb:00fc** ("Prometheus Fingerprint Reader"). It is in the repository's installer-extras.json, but
      not in the copy on this laptop (installed from the 2026-11-12 ISO, before the list was added). Nothing installed or enrolled.
- [ ] Crashes seen 2026-11-13 (coredumpctl): avahi-daemon aborted 4 times, each at a Wi-Fi change, inside its own shutdown
      (avahi_server_free): heimdall-wifi-guard stops it on untrusted networks, and the abort leaves the unit failed. Harmless
      (it is meant to be off there), but a quieter way to switch it off would be tidier. hyprsunset (Night Light) aborted once
      around a screen-scale change and was restarted by the panel.
- [ ] thermald: not for this laptop (its firmware runs the cooling; thermald stops itself where
      `/sys/devices/platform/thinkpad_acpi/dytc_lapmode` exists). Check that file exists here, so the installer's rule is right.
      2026-11-13: it exists (it read 1: lap mode), so the rule is right. Settings > Power > Cooling shows the firmware's two fans,
      and the thermal profile follows the power mode (platform_profile: low-power, balanced, performance).
- [ ] Firmware updates: fwupd is **not** on the ISO. ThinkPads get BIOS, Thunderbolt and fingerprint firmware from LVFS. Write
      down what `fwupdmgr get-devices` would cover (install fwupd only with my OK). Firmware updates go through Agora/Settings later.
      2026-11-13: fwupd is not installed, so `get-devices` waits for your OK.
- [ ] Secure Boot later: sbctl is on the ISO. Our own keys and signed kernels are a later plan; don't enrol keys now.

## Later: LykanOS on this laptop

The laptop's long-term system is LykanOS (Arch + BlackArch on Yggdrasil). Its ISO is built from the LykanOS repository, and it
takes Yggdrasil, Sarina and Heimdall from the same `[labyrinth]` repo. What is learned here (drivers, rotation, fingerprint,
suspend) goes into Yggdrasil and the installer's components, so both systems get it. Decided 2026-11-12: LykanOS comes
onto this laptop only after Labyrinth is fine-tuned here and the Rust porting is done; then LykanOS is built out and installed
(with disk encryption). Until then the laptop stays on Labyrinth.

## Rules for the laptop session

- The repositories are public: no serial numbers, MAC addresses, host names, network names, e-mail addresses or anything personal.
  Docs are written in the first person, as me, with no name.
- `git pull --rebase` before every push; stage files by path; one CI watch per push.
- Ask me before installing packages, changing the BIOS, enrolling a fingerprint, or changing PAM, encryption or Secure Boot.
- Results go into this file (checkboxes and notes), and code fixes go into Yggdrasil or Labyrinth_OS as usual.
