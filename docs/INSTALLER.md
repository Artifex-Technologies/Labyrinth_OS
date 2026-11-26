# Installer

> **Superseded (2026-10-24):** Labyrinth no longer uses Calamares. It has its own installer: Yggdrasil's installer window
> (`yggdrasil-installer`) with Labyrinth's install steps (`labyrinth-install-system`; since 2026-11-07 Yggdrasil's
> `yggdrasil-install-system`, configured by `labyrinth-installer.conf`), including manual partitioning beside Windows. Calamares itself was removed from the ISO on 2026-11-07. The current description is Yggdrasil's
> [docs/INSTALLER.md](https://github.com/Artifex-Technologies/Yggdrasil/blob/main/docs/INSTALLER.md). What follows is the earlier
> Calamares plan, kept for its reasoning.

Decision: **Calamares, copying the live image onto the disk**, the same model as
Chymaera_OS (see its `docs/INSTALLER.md` for the long reasoning). It is fast, needs
no network, and the config is already proven statically. Not chosen: CachyOS's own
installer (not examined; it would be a second installer stack to maintain) and
`archinstall` (TUI, pacstrap-based, needs a network).

## Status

| Piece | State |
| --- | --- |
| Calamares job sequence, module configs | Written. Module list checked against what the pinned build produces; every YAML file parses. **Never run** |
| Module configs vs Calamares 3.4.2 samples | Generated from the real 3.4.2 samples (hash-verified tarball); the diff against upstream is exactly the intended lines |
| `labyrinth-install-cleanup` | **Tested** against a mock target root: 20 checks (three kernels, missing fallback kernel, no kernel, bad preset, hostile name) |
| `labyrinth-install-bootfinish` | Syntax-checked only. **Never run**; snapper/grub-btrfs/zram steps follow their documentation |
| Calamares package | Vendored PKGBUILD, source hash re-verified. **Not built** |
| Whole install, boot, login | **Never done.** Needs a built ISO in a VM |

## Flow

```
partition (erase disk) -> mount (btrfs subvolumes) -> unpackfs (copy live squashfs)
 -> cleanup (kernels, presets, live-only files) -> machineid/locale/keyboard/fstab
 -> initcpiocfg, initcpio (all kernels) -> users -> displaymanager -> networkcfg
 -> hwclock -> services-systemd -> grubcfg -> bootloader (GRUB, UEFI)
 -> bootfinish (verify grub.cfg, zram, snapper + grub-btrfs) -> umount
```

## Choices for this machine

| Choice | Setting | Why |
| --- | --- | --- |
| Firmware | UEFI only | Board boots UEFI; BIOS support dropped from `profiledef.sh` |
| Partitioning | Erase the disk by default; ESP 1 GiB at `/boot/efi`; rest btrfs | You are replacing Windows. Manual partitioning is still available in the wizard |
| Subvolumes | `@`, `@home`, `@cache`, `@log` (Calamares' defaults) | Snapshots of `@` do not drag logs/caches along |
| Mount options | `noatime`, `compress=zstd:1` | Fewer writes, free space |
| Swap | zram `min(ram/2, 16 GiB)` plus a swap file the size of RAM (~64 GiB) in a nested `/swap` btrfs subvolume | Hibernate. `labyrinth-install-bootfinish` creates it, adds `resume=`/`resume_offset=` and the initramfs `resume` hook, and sets NVIDIA to keep video memory. **Untested until the first VM install**; if any step fails the power menu just omits Hibernate |
| Bootloader | GRUB + grub-btrfs | Calamares supports it as shipped, and it can read btrfs, so kernels live inside the snapshotted root and a rollback never leaves kernel and modules out of step. systemd-boot cannot do that with kernels on the ESP; Limine needs a patched Calamares |
| Kernel cmdline | `quiet nvidia_drm.modeset=1 nvidia_drm.fbdev=1` | NVIDIA on Wayland |
| GRUB timeout | 2 s | Faster boot; hold Esc/Shift for the menu |
| Kernels | `linux-cachyos` and `-lts` both get presets, images and entries | `-lts` is the fallback; cleanup warns if it is missing |
| Graphics drivers | Only those the computer's chips need: `labyrinth-hardware-setup` (at the install's end, and at each start where the display controllers differ from its record in `/var/lib/labyrinth/hardware-chips`) installs what is missing (Mesa always; Intel: `vulkan-intel`, `intel-media-driver`; AMD: `vulkan-radeon`; NVIDIA: the open module for each kernel, `nvidia-utils`, `nvidia-settings`, its sleep services; the 32-bit Vulkan driver where Steam is installed; GPU switching with two chips), then removes the other vendors'. Offline it removes nothing and tries again at the next start | The medium carries every driver to install offline; a driver nobody uses is still updated and takes room (1.6 GB of NVIDIA's on an Intel-only laptop) |
| Start-up image firmware | Only the NVIDIA open driver's firmware for the card's generation: `labyrinth-gpu-firmware` (a mkinitcpio hook, added by `/etc/mkinitcpio.conf.d/labyrinth-gpu-firmware.conf`) puts an empty file in the image in place of each other generation's GSP and microcode file, since mkinitcpio adds a module's firmware after the hooks and skips a file the image has. Never on the live medium; both generations stay with no NVIDIA card, both kinds, or a card older than Turing | The driver loads early for the boot animation; with a Turing card the image drops from 223 MB to 170 MB (measured on a build of my PC's image), which GRUB reads at every start |
| Root account | Locked; the user is in `wheel` and uses sudo | |
| Hostname | `labyrinth` | |
| Min. disk / RAM | ~30 GB / ~4 GB | Both are guesses; the installed size is unmeasured |

## Secure Boot (why it is off)

**Disable it in the BIOS before installing.** Reasons, none of them tested here:

- **NVIDIA modules.** With Secure Boot the kernel enforces module signing (lockdown).
  The prebuilt `linux-cachyos-*-nvidia-open` modules, and anything built by DKMS,
  would have to be signed with a key the firmware trusts, or the GPU driver will not
  load. Whether CachyOS's own signing is accepted depends on its kernel config, which
  I have not checked. This is the classic Secure Boot + NVIDIA problem on Arch.
- **GRUB.** The clean sbctl route signs a single unified kernel image and boots it with
  systemd-boot (or Limine). GRUB under Secure Boot needs either shim or a self-signed
  GRUB with its modules built in, and the kernel is then not verified by GRUB. That is
  workable but fiddly, and it is the bootloader chosen for snapshot-safe rollback.
- **No benefit for your games.** Linux anti-cheat support (EAC/BattlEye via Proton)
  does not check Secure Boot, and nothing here needs it.
- **Windows 11 does not need it** to run, and you are replacing Windows.

When it would be worth doing: if you keep a small Windows partition for a game whose
anti-cheat requires Secure Boot to be on (Vanguard and FACEIT are the known examples).
Then toggling it in the BIOS each time gets old. The route would be:
firmware in Setup Mode -> `sbctl create-keys` -> `sbctl enroll-keys --microsoft` (keep
Microsoft's keys, or your GPU's firmware and Windows stop booting) -> sign the
bootloader and kernels -> sign the NVIDIA modules, probably moving to systemd-boot with
a signed unified kernel image and giving up GRUB snapshot entries. Labyrinth does not set this up.

## Testing the cleanup program yourself

    (cd packages/tools && cargo build --release) && bash tests/test_install_cleanup.sh
