# Hardware: what Labyrinth OS does with particular machines

Written 2026-11-12. One section per machine I have tested on: what works, which parts of the system and which optional components
cover it, and the firmware (BIOS) settings it needs. The checklist for testing on a machine lives in its own plan (for the
laptop: [LAPTOP.md](LAPTOP.md)); this file is the reference once something is known.

Device software is never on the ISO (it stays small): the installer's **Optional software** page ticks what fits the machine and
installs it at the first login, online; Agora has the same components afterwards. See [COMPONENTS.md](COMPONENTS.md).

## ThinkPad X1 Yoga Gen 8 (2-in-1)

13th-gen Intel Core with Iris Xe graphics, 14-inch touchscreen with a Wacom pen, TrackPoint and touchpad, fingerprint reader in
the power button, Intel AX211 Wi-Fi 6E and Bluetooth, accelerometer for tablet mode. My LykanOS machine, running Labyrinth OS
1.0 for now.

### Firmware (BIOS) settings

| Setting | Value | Why |
| --- | --- | --- |
| Secure Boot | **Off** | Our kernels and boot loader are not signed yet. `sbctl` is on the ISO for later; our own keys are a separate plan (do not enrol keys now). |
| Intel AMT | **Disabled** | Remote management in the chipset that works below the operating system; nothing here uses it, and it is attack surface. |
| Sleep state | Linux / s2idle (the default on this generation) | `cat /sys/power/mem_sleep` should show `[s2idle]`. |

### What the system has for it

| Part | How | Where |
| --- | --- | --- |
| Graphics | Mesa (`vulkan-intel`, `intel-media-driver` for video decoding) | on the ISO |
| Wi-Fi and Bluetooth (AX211) | `iwlwifi` firmware in `linux-firmware`, NetworkManager, BlueZ | on the ISO |
| Sound | `sof-firmware` and `alsa-ucm-conf` (Intel's audio DSP) | on the ISO; to be checked (LAPTOP.md) |
| Battery | Top bar battery, power mode (power-profiles-daemon), charge limit 60/80/90/100 % | Yggdrasil `c1d06b7`, `8d428b1`, `f3ac236` |
| Fans and temperature | Run by the firmware; Settings > Power > Cooling shows them, read only (my choice) | Yggdrasil `0d23a97` |
| Power modes | power-profiles-daemon switches the ThinkPad's platform profile (DYTC: the firmware's own cooling and power limits) and the processor's energy preference | on the ISO |
| thermald | **Not used on this laptop.** Its own code refuses to run on ThinkPads with `/sys/devices/platform/thinkpad_acpi/dytc_lapmode` ("Some Lenovo machines have in-firmware thermal management, avoid having two entities trying to manage things"), so the installer does not tick it here. On other Intel laptops it is the `thermald` component, ticked automatically; it reads power-profiles-daemon's mode as its "power slider", so the two work together, not against each other. | component `thermald` |
| TrackPoint and touchpad | Middle button + TrackPoint scrolls; touchpads scroll naturally | Yggdrasil `572bb64`, `90f08d9` |
| Screen scale | A laptop's own screen (eDP) gets Hyprland's automatic scale unless Settings > Displays set one | Yggdrasil `34817e8` |
| Touchscreen and pen | Mapped to the built-in screen (Wacom HID digitiser), so an external monitor does not stretch them | Yggdrasil `3d21b74` |
| Tablet mode and rotation | Folding the screen back flips the tablet-mode switch (thinkpad_acpi's "ThinkPad Extra Buttons"); `/usr/lib/yggdrasil/tablet watch` then turns the screen with the accelerometer (iio-sensor-proxy, on the ISO), the touchscreen and pen with it, and shows the on-screen keyboard while no keyboard is plugged in. Settings > Displays > Tablet mode: Lock rotation, On-screen keyboard. On Hyprland only (Yggdrasil's own compositor cannot turn a screen yet). | Yggdrasil `934c24e` |
| On-screen keyboard | wvkbd from the AUR | component `on-screen-keyboard`, ticked when the computer has a tablet-mode switch |
| Fingerprint reader | fprintd + libfprint; Settings > Users > Fingerprint: add a finger, remove them, **Use for sudo** (off until I turn it on; an administrator's password to change; the password always works too). The lock screen and password boxes still take the password only (planned: Yggdrasil docs/DESKTOP.md). | component `fingerprint`, ticked when a reader libfprint supports is plugged in; Yggdrasil `934c24e` |
| Firmware updates | Not yet: fwupd is not on the ISO. ThinkPads get BIOS, Thunderbolt and fingerprint-reader firmware from LVFS. | later, through Agora/Settings |

### The fingerprint reader

The X1 Yoga Gen 8's reader is a USB device from Synaptics (`06cb`) or Goodix (`27c6`); which one is on my laptop is still to be
read with `lsusb` (LAPTOP.md). Both vendors also make readers libfprint cannot drive, so the installer ticks the component by the
exact device: the list in `installer-extras.json` is every USB reader libfprint 1.94.100 supports (its udev hwdb, the "Known
unsupported devices" section left out). Readers on SPI (some Elan ones) are not in that list; they are added from Agora.

Installing fprintd changes nothing in PAM. Fingerprint for sudo adds exactly two lines at the top of `/etc/pam.d/sudo` (a marker
and `auth sufficient pam_fprintd.so`) through a root helper, after an administrator's password; turning it off removes exactly
those two lines. "sufficient" means a finger lets sudo in and no finger falls through to the password, so it cannot lock anyone
out. `/etc/pam.d/sudo` is a backup file of the `sudo` package, so an update keeps the change (a `.pacnew` appears if the package's
own copy changes).

### Rotation directions

The screen's turn follows iio-sensor-proxy's reading the way GNOME does: `normal` upright, `left-up` turned 90°, `bottom-up` 180°,
`right-up` 270° (Hyprland transforms 0 to 3). Whether the direction and the touch turn are right on this panel is on the laptop's
checklist.
