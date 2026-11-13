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

## In progress on the desktop PC (do not start these here)

- **Location without Wi-Fi.** I don't want nearby Wi-Fi networks or their hardware IDs used to find where the computer is. The
  automatic time zone and Weather's "Automatic" place are moving to IP-only, and they pause while a VPN is on. Until that update
  arrives, leave "Set time zone automatically" **off**.
- **Laptop hardware set:** fingerprint reader (fprintd as an online component; turning it on for login and sudo is my choice in
  Settings, off by default), thermald, screen rotation from the accelerometer, tablet mode with an on-screen keyboard, and a
  hardware page in `docs/HARDWARE.md`. When that work lands, test it here (below).
- **Review of the commits above.** Another session reviews this laptop session's commits for security and for anything that breaks
  on a desktop without a battery. If it changes something you wrote, pull before you edit those files again.

## To check on the laptop (needs the hardware)

Write what you find under each item, and what works without changes.

Basics:
- [ ] Suspend and resume (lid close, power key, idle): comes back with Wi-Fi, sound and touch. Check `cat /sys/power/mem_sleep`
      (s2idle is expected) and battery drain overnight while suspended.
- [ ] Lid closed with an external screen attached: stays awake and uses only the external screen.
- [ ] Brightness keys, volume keys, mic-mute key and its light, the F-keys' other functions (Fn lock)
- [ ] Sound: speakers and headphone jack (sof-firmware and alsa-ucm-conf are on the ISO); the microphones
- [ ] Camera: check whether it is a USB camera (`lsusb`) or a MIPI camera on Intel IPU6 (`lspci`; it would need the IPU6 driver
      and libcamera). Write down which, and don't install anything for it yet.
- [ ] External screens over USB-C / Thunderbolt and HDMI: plug in and out while running; the dock and windows follow
- [ ] Thunderbolt devices are authorised (bolt is not on the ISO; check whether anything is needed)
- [ ] Battery: charge limit really stops charging at the set level; power modes change the fans and the battery life
- [ ] VS Code's workspace-trust dialog (and its other pop-ups) fits the screen at my scale, centred, with nothing to drag
      around (Yggdrasil `d058852`: no floating window opens bigger than its screen). Also check a window kept from before a
      scale change in Settings > Displays comes back no bigger than the screen.

2-in-1:
- [ ] Pen: pressure, the side buttons, the eraser end, palm rejection while writing
- [ ] Touch: tap, scroll, pinch in apps; touch on the right screen with an external one attached
- [ ] Rotation and tablet mode: once the desktop PC's work lands, folding into tablet mode rotates the screen, maps touch and pen
      with it, and shows the on-screen keyboard; the rotation lock works

Security (this is the LykanOS machine):
- [ ] Disk encryption: was it chosen in the installer? If not, write it down. Reinstalling with encryption comes with LykanOS.
- [ ] Heimdall is running (`systemctl status heimdall`), and its alerts make sense on a laptop moving between networks
- [ ] The firewall (ufw) is on; Wi-Fi checks on public networks (Settings > Network)
- [ ] Fingerprint reader: which one it is (`lsusb`: Synaptics 06cb or Goodix 27c6); once the component lands, enrol a finger and
      test login and sudo (only with my OK)
- [ ] Firmware updates: fwupd is **not** on the ISO. ThinkPads get BIOS, Thunderbolt and fingerprint firmware from LVFS. Write
      down what `fwupdmgr get-devices` would cover (install fwupd only with my OK). Firmware updates go through Agora/Settings later.
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
