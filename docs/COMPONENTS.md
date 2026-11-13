# Optional components

Labyrinth ships a base system and a list of **components**: software that is yours to
choose, so someone else's install need not carry what you use. The list is
`profile/airootfs/usr/share/labyrinth/components.json`. Agora installs them (`labyrinth-apps`); a component whose
software Labyrinth builds comes from Labyrinth's signed package repository ([PACKAGE_REPO.md](PACKAGE_REPO.md)), and is built on
the machine only when that cannot be reached.

| Component | What it is | Notes |
| --- | --- | --- |
| `steam` | Steam, Valve's Proton, Proton-CachyOS, ProtonUp-Qt, Protontricks, umu, GameMode, MangoHud, 32-bit NVIDIA/Vulkan (no system Wine: Proton has its own) | `proton-cachyos` and `protonup-qt` are not in Arch's repos (CachyOS repo or AUR) |
| `spotify` | `spotify-launcher`, which downloads Spotify's own client | Spotify's [Linux page](https://www.spotify.com/us/download/linux/) offers only Snap and a Debian repo and says Linux is "not a platform that we actively support". `spotify-launcher` is Arch's way to run that same official client without Snap |
| `obsidian` | Obsidian, from Arch's repo | Proprietary, free for personal use |
| `weather` | Labyrinth's multi-model weather engine and its hourly recording timer (see [WEATHER.md](https://github.com/Artifex-Technologies/Yggdrasil/blob/main/docs/WEATHER.md)) | Optional: the bar's weather slot hides itself when it is not installed |
| `vscode` | Microsoft's official VS Code, from the AUR's `visual-studio-code-bin` (Microsoft's own release, downloaded from Microsoft; Agora's updates keep it current) | Microsoft's licence does not allow passing it on, so it is not in Labyrinth's repository. Arch's `code` is the open-source build with a different marketplace; the two conflict |
| `containers` | Podman + Distrobox: Fedora/Debian/Ubuntu userlands whose apps show up in the app menu | See below |
| `flatpak` | Flatpak (Flathub) | See below |
| `proton-drive` | rclone + FUSE, mount service, `minos-cloud` links | No official Linux client exists |
| `proton-pass` | Proton Pass, from the AUR's `proton-pass-bin` (Proton's own release) | Agora's updates keep it current |
| `proton-mail` | Proton Mail (and Calendar), from the AUR's `proton-mail-bin` | Bridge for other mail programs: `protonmail-bridge` (Arch) |
| `creative` | Darktable and OBS Studio | Krita, GIMP, Ardour and more are in Agora one by one |
| `virtualization` | QEMU, libvirt, UEFI firmware, dnsmasq, nftables (for Limbo) | |
| `dev` | Node.js, npm, jq, the GitHub CLI, pip, fd, bubblewrap and socat: what Claude Code and most command-line development want (git is in the base system) | See [CLAUDE_CODE.md](CLAUDE_CODE.md); on in every preset except `minimal` |
| `sarina` | Sarina, with her reverse-engineering tools (Java for Ghidra, Node for REA, bubblewrap, binutils) | Optional; the installer offers her |
| `heimdall` | Heimdall, with ClamAV (its malware scanning) and ClamAV's signature updates | Optional; the installer offers it. It dissects malware inside Limbo's sealed guest, not on the host |
| `kairos` | Kairos, Labyrinth's own frame generation (preview; `labyrinth-framegen` until 2026-11-07): a Vulkan layer for 64- and 32-bit games and its launcher | Settings > Games chooses games and 2x/3x/4x |
| `frame-generation` | lsfg-vk (Lossless Scaling's frame generation), from its source in the AUR | Needs Lossless Scaling from Steam; CC BY-NC-ND, so not part of Labyrinth itself |
| `daedalus-cad` | Daedalus CAD, Artifex's SketchUp-style modeler | From the package repository |
| Devices: `logitech`, `gaming-mice`, `razer`, `ckb-next`, `xbox-controllers`, `hp-printers` | Solaar; Piper + libratbag; OpenRazer; ckb-next (Corsair); xpadneo; HPLIP | Not on the ISO: the installer offers them (ticked when such a USB device is plugged in) and installs them at the first login. Razer and xpadneo bring dkms and both kernels' headers, which only drivers built on the machine need |
| `local-ai` | Ollama for Sarina (CUDA on NVIDIA, Vulkan otherwise) | Not in the image (~6 GB with CUDA); Sarina offers to install it on first use. Competes with games for GPU memory |

**Presets:** `minimal` (nothing extra), `gaming` (steam), `creative` (creative +
obsidian), `everything` (all of the above including weather; your own setup). Spotify is in no preset and Obsidian only in
`creative` (2026-11-13): neither is part of the OS, and Agora installs both.

## Software that only lists Fedora or Debian

Most of that is a packaging format, not a real incompatibility: a `.rpm` or `.deb` is an
archive around binaries that usually run fine on Arch. Options, best first:

1. **Arch's own repo or the AUR.** Check there first; for example `code`, `obsidian`,
   `spotify-launcher` and `steam` are in Arch's repo.
2. **Repackage the vendor's official file with a pinned PKGBUILD.** Download the vendor's
   file, check its published hash, install its contents. Works when the app is a
   self-contained binary. (Proton Pass and VS Code were done this way; the AUR's `-bin`
   packages now do the same, with updates through Agora.)
3. **Flatpak** (component `flatpak`): a distro-neutral format; many apps publish on Flathub.
4. **Distrobox** (component `containers`): a real Fedora or Debian environment running
   inside your desktop. Apps from it can be exported to the app menu and share your home
   folder. `--nvidia` passes the GPU driver in. This is the closest thing to "running Fedora
   apps in Arch": it is not Fedora's package system on the host, it is a container.
   Not tested here; GPU-heavy apps (games, CAD) belong on the host, not in a container.

Not recommended: converting packages with tools like `alien` (not in Arch's repos),
installing `rpm` or `dpkg` to unpack and scatter files over the system, or mixing Fedora's
repos into pacman. They tend to break on library versions, and uninstalling leaves a mess.

## Two ways to use them

- **On an installed system:** Agora, or `labyrinth-apps list | info ID | install ID... | remove ID...`
  (add `--dry-run` to see the commands). It uses pacman (Labyrinth's signed repository for its own
  software), yay for the AUR packages (Proton Pass, Proton Mail, VS Code: yay shows each build file
  and asks first), and makepkg for a recipe only when the repository cannot be reached. It needs the
  network and the administrator password.
- **When building an ISO:** `scripts/gen-packages.py --preset everything` (or
  `--preset minimal`, or `--components steam,obsidian`) writes `profile/packages.x86_64`
  from `profile/packages.base` plus the chosen components. Choosing at build time puts
  the software on the ISO, so the install works offline. The committed file is the
  `everything` preset, which is what your ISO would be; `python tests/test_labyrinth_apps.py`
  fails if the committed file goes stale.

## Choosing at install time

The installer's **Optional software** page (`/usr/share/labyrinth/installer-extras.json`) offers Sarina and Heimdall under "AI
and security", and the device components under "Devices", each ticked when such a USB device is plugged in. What the medium
does not carry is installed at the first login, online (`labyrinth-first-components`). Its **Browsers** page
(`installer-browsers.json`) does the same for browsers.

## Not done

- Sizes are not listed: none has been measured.
