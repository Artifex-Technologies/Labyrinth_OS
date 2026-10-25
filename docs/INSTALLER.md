# Installer

> **Superseded (2026-10-24):** Labyrinth no longer uses Calamares. It has its own installer: Yggdrasil's installer window
> (`yggdrasil-installer`) with Labyrinth's install steps (`profile/airootfs/usr/local/bin/labyrinth-install-system`), including
> manual partitioning beside Windows. The current description is Yggdrasil's
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
| Root account | Locked; the user is in `wheel` and uses sudo | |
| Hostname | `labyrinth` | |
| Min. disk / RAM | ~30 GB / ~4 GB | Both are guesses; the installed size is unmeasured |

## Not done

- **Never run.** First test: build the ISO in WSL (Arch), boot it in a VM, install, reboot,
  log in. Most likely fixes: the unpack path, the keyring step, GRUB on UEFI, whether
  `snapper create-config` works in the installer chroot (it may not; then snapshots
  are skipped with a warning and set up on first boot instead).
- **Branding** is upstream's sample (squid logo included).
- **Secure Boot**: not handled. Plan is to turn it off; see the next section.
- **Disk encryption** is whatever the stock wizard offers; GRUB-unlocked LUKS is untested here.
- **Dual boot**: `os-prober` is installed and enabled, so Windows and other systems already on the disks get a boot menu entry (with their emblem in the Labyrinth boot theme). The installer's Partitions page still has no "install alongside" wizard: shrink the disk by hand first (see ARCHITECTURE.md). Untested here with a real Windows disk.
- **Agent-assisted install** (Sarina) does not exist yet.
- NVIDIA early module loading in the initramfs is deliberately off (see `grubcfg.conf`
  for what is set); add it only if the console flickers at boot.

## Secure Boot

**Plan: disable it in the BIOS before installing.** Reasons, none of them tested here:

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
a signed unified kernel image and giving up GRUB snapshot entries. Not built.

## Testing the cleanup script yourself

    bash tests/test_install_cleanup.sh
