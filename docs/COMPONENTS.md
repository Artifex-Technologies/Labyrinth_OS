# Optional components

Labyrinth ships a base system and a list of **components**: software that is yours to
choose, so someone else's install need not carry what you use. The list is
`profile/airootfs/usr/share/labyrinth/components.json`.

| Component | What it is | Notes |
| --- | --- | --- |
| `steam` | Steam, Valve's Proton, Proton-CachyOS, ProtonUp-Qt, Protontricks, umu, Wine, GameMode, MangoHud, 32-bit NVIDIA/Vulkan | `proton-cachyos` and `protonup-qt` are not in Arch's repos (CachyOS repo or AUR) |
| `spotify` | `spotify-launcher`, which downloads Spotify's own client | Spotify's [Linux page](https://www.spotify.com/us/download/linux/) offers only Snap and a Debian repo and says Linux is "not a platform that we actively support". `spotify-launcher` is Arch's way to run that same official client without Snap |
| `obsidian` | Obsidian, from Arch's repo | Proprietary, free for personal use |
| `weather` | Labyrinth's multi-model weather engine and its hourly recording timer (see [WEATHER.md](WEATHER.md)) | Optional: the bar's weather slot hides itself when it is not installed |
| `vscode` | Microsoft's official VS Code, repackaged from their tarball, pinned and hash-checked (verified) | Their [Linux docs](https://github.com/microsoft/vscode-docs/blob/main/docs/setup/linux.md) cover Debian, Fedora, Snap and a community AUR package; nothing official for Arch. Arch's `code` is the open-source build with a different marketplace; the two conflict |
| `containers` | Podman + Distrobox: Fedora/Debian/Ubuntu userlands whose apps show up in the app menu | See below |
| `flatpak` | Flatpak (Flathub) | See below |
| `proton-drive` | rclone + FUSE, mount service, `labyrinth-cloud` links | No official Linux client exists |
| `proton-pass` | Proton's official desktop app, repackaged from their RPM, pinned and hash-checked | Built on the machine with makepkg |
| `creative` | Krita, Darktable, Kdenlive, OBS Studio, Ardour | |
| `virtualization` | QEMU, libvirt, UEFI firmware, dnsmasq, nftables (for Limbo) | |
| `dev` | Node.js, npm, jq, the GitHub CLI, pip, fd, bubblewrap and socat: what Claude Code and most command-line development want (git is in the base system) | See [CLAUDE_CODE.md](CLAUDE_CODE.md); on in every preset except `minimal` |
| `local-ai` | Ollama for Sarina (CUDA on NVIDIA, Vulkan otherwise) | Not in the image (~6 GB with CUDA); Sarina offers to install it on first use. Competes with games for GPU memory |

**Presets:** `minimal` (nothing extra), `gaming` (steam), `creative` (creative +
obsidian), `everything` (all of the above including weather; your own setup).

## Software that only lists Fedora or Debian

Most of that is a packaging format, not a real incompatibility: a `.rpm` or `.deb` is an
archive around binaries that usually run fine on Arch. Options, best first:

1. **Arch's own repo or the AUR.** Check there first; for example `code`, `obsidian`,
   `spotify-launcher` and `steam` are in Arch's repo.
2. **Repackage the vendor's official file with a pinned PKGBUILD.** What we do for
   Proton Pass and VS Code: download the vendor's file, check its published hash, install
   its contents. Works when the app is a self-contained binary.
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

- **On an installed system:** `labyrinth-apps list | info ID | install ID... | remove ID...`
  (add `--dry-run` to see the commands). It uses pacman, and makepkg for `proton-pass`,
  so it needs network and sudo.
- **When building an ISO:** `scripts/gen-packages.py --preset everything` (or
  `--preset minimal`, or `--components steam,obsidian`) writes `profile/packages.x86_64`
  from `profile/packages.base` plus the chosen components. Choosing at build time puts
  the software on the ISO, so the install works offline. The committed file is the
  `everything` preset, which is what your ISO would be; `python tests/test_labyrinth_apps.py`
  fails if the committed file goes stale.

## Not done

- **A chooser in the installer.** Calamares can show a component page, but its
  `packagechooser` module is switched off in our package build, and installing chosen
  components at install time needs either network or an ISO that already contains them.
  For now: build the ISO you want, or install components after first boot.
- **PKGBUILDs under `/usr/share/labyrinth/pkgbuilds/`.** `labyrinth-apps` builds
  `proton-pass` from there, but nothing copies `packages/pkgbuilds/` into the image yet
  (no build script exists).
- **A graphical front end** (a "Labyrinth Apps" window, or a tab in the file explorer).
- Sizes are not listed: none has been measured.
