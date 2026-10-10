#!/usr/bin/env bash
# Build a small bootable Arch disk image containing the Labyrinth desktop, to boot in QEMU.
# This is a way to SEE and test the desktop before the real ISO exists. It is NOT the ISO:
# stock Arch kernel (no CachyOS repos), no installer, no CachyOS/NVIDIA parts.
#
# Run as root on Arch (WSL works):   sudo scripts/vm/build-vm-image.sh
# Needs: arch-install-scripts e2fsprogs qemu-desktop. Writes to $OUT (default /root/vm).
# No loop devices or mounts of the image are used: the filesystem is created from a directory.
set -euo pipefail

OUT=${OUT:-/root/vm}
SIZE=${SIZE:-10G}
ROOTFS=$OUT/rootfs
IMG=$OUT/labyrinth.img
REPO=$(cd "$(dirname "$0")/../.." && pwd)
PKGDIR=${PKGDIR:-}          # directory holding built labyrinth-*.pkg.tar.zst files (optional)
DEMO_LAT=${DEMO_LAT:-40.71} # weather DEMO location only (New York). Not a real user's location.
DEMO_LON=${DEMO_LON:--74.01}

PKGS=(base linux mkinitcpio hyprland waybar nwg-dock-hyprland foot fuzzel mako uwsm hyprpolkitagent
      cliphist grim slurp wl-clipboard playerctl pipewire pipewire-pulse wireplumber
      ttf-jetbrains-mono-nerd noto-fonts noto-fonts-emoji kvantum xdg-utils curl libnotify
      swaybg adwaita-icon-theme pavucontrol btop python mesa vulkan-swrast vulkan-icd-loader bash-completion nano)

step() { printf '\n== %s\n' "$*"; }

step "pacstrap (this downloads a few hundred MB the first time)"
# SAFETY: pacstrap/arch-chroot bind-mount /dev, /proc, /sys and /run under the target. If an earlier
# run was killed, those mounts can still be there, and `rm -rf` would then delete through them into
# the REAL /dev (this happened once and broke the WSL distro until it was restarted). So: unmount
# everything under the target first, and refuse to delete if any mount remains.
if [[ -d $ROOTFS ]]; then
  umount -R "$ROOTFS" 2>/dev/null || true
  if findmnt -rn -R "$ROOTFS" 2>/dev/null | grep -q .; then
    echo "error: mounts still exist under $ROOTFS; refusing to delete. Reboot WSL (wsl --shutdown) and retry."
    exit 1
  fi
fi
rm -rf --one-file-system "$ROOTFS"; mkdir -p "$ROOTFS" "$OUT"
pacstrap -K -c "$ROOTFS" "${PKGS[@]}"

step "Labyrinth packages and files"
if [[ -n $PKGDIR ]] && ls "$PKGDIR"/labyrinth-*.pkg.tar.zst >/dev/null 2>&1; then
  # /var/tmp, not /tmp: arch-chroot mounts a fresh tmpfs over /tmp, hiding anything copied there.
  mkdir -p "$ROOTFS/var/tmp/lpkgs"
  cp "$PKGDIR"/labyrinth-finder-*.pkg.tar.zst "$PKGDIR"/labyrinth-weather-[0-9]*.pkg.tar.zst "$ROOTFS/var/tmp/lpkgs/" 2>/dev/null || true
  # CheckSpace cannot work in a chroot whose root is not its own mount ("could not determine root mount point").
  sed -i 's/^CheckSpace/#CheckSpace/' "$ROOTFS/etc/pacman.conf"
  arch-chroot "$ROOTFS" bash -c 'pacman -U --noconfirm --needed /var/tmp/lpkgs/*.pkg.tar.zst'
  sed -i 's/^#CheckSpace/CheckSpace/' "$ROOTFS/etc/pacman.conf" || { echo "error: local packages not installed"; exit 1; }
  rm -rf "$ROOTFS/var/tmp/lpkgs"
fi
SRC=$REPO/profile/airootfs
install -d "$ROOTFS/usr/local/bin" "$ROOTFS/usr/share/labyrinth"
for f in "$SRC"/usr/local/bin/labyrinth-{shell,quick-settings,floats,keys,displays,apps,workspace}; do install -m755 "$f" "$ROOTFS/usr/local/bin/"; done
cp -a "$SRC/usr/share/labyrinth/." "$ROOTFS/usr/share/labyrinth/"
install -m755 "$REPO/scripts/vm/vmtest-hypr.sh" "$ROOTFS/usr/local/bin/vmtest-hypr"   # run inside the VM to probe hyprctl
# the explorer's desktop entry (the package built before this existed does not ship it)
install -Dm644 "$REPO/packages/finder/finder.desktop" "$ROOTFS/usr/share/applications/finder.desktop"

step "user, login, desktop config"
arch-chroot "$ROOTFS" bash -s <<'CH'
set -e
echo labyrinth-vm > /etc/hostname
sed -i 's/^#en_US.UTF-8/en_US.UTF-8/' /etc/locale.gen && locale-gen >/dev/null
echo LANG=en_US.UTF-8 > /etc/locale.conf
useradd -m -G wheel,video,audio,input -s /bin/bash labyrinth
echo 'labyrinth:labyrinth' | chpasswd; echo 'root:labyrinth' | chpasswd
mkdir -p /etc/systemd/system/getty@tty1.service.d
# DHCP on the VM's wired NIC, so the bar's network and weather indicators have something to show.
systemctl enable systemd-networkd systemd-resolved >/dev/null 2>&1
mkdir -p /etc/systemd/network
printf '[Match]\nName=en*\n\n[Network]\nDHCP=yes\n' > /etc/systemd/network/20-wired.network
printf '[Service]\nExecStart=\nExecStart=-/sbin/agetty --autologin labyrinth --noclear %%I $TERM\n' > /etc/systemd/system/getty@tty1.service.d/autologin.conf
CH
# Outside the chroot: arch-chroot bind-mounts its own /etc/resolv.conf over the one inside.
ln -sf ../run/systemd/resolve/stub-resolv.conf "$ROOTFS/etc/resolv.conf"
cp -a "$SRC/home/live/.config" "$ROOTFS/home/labyrinth/.config"
cat > "$ROOTFS/home/labyrinth/.bash_profile" <<'EOF'
# Test image: start the desktop on the first console. Software rendering because the VM has no GPU.
if [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(tty)" = /dev/tty1 ]; then
  export LIBGL_ALWAYS_SOFTWARE=1 WLR_NO_HARDWARE_CURSORS=1 XDG_RUNTIME_DIR=/run/user/$(id -u)
  mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
  exec Hyprland > ~/hyprland-stdout.log 2>&1
fi
EOF
# A demo weather location so the bar's weather slot has something to show. Clearly labelled.
install -d "$ROOTFS/home/labyrinth/.config/labyrinth"
printf '{"name":"Demo location (New York)","lat":%s,"lon":%s,"units":"metric"}\n' "$DEMO_LAT" "$DEMO_LON" > "$ROOTFS/home/labyrinth/.config/labyrinth/weather.json"
arch-chroot "$ROOTFS" chown -R labyrinth:labyrinth /home/labyrinth

step "disk image (no mounting: filesystem built from the directory)"
cp "$ROOTFS/boot/vmlinuz-linux" "$OUT/vmlinuz-linux"
# Arch no longer ships a fallback initramfs, and the default one uses 'autodetect', which trims
# it to the machine that built it (here WSL, which has no virtio disk). Build a VM-specific one
# with the drivers listed explicitly and no autodetect.
cat > "$ROOTFS/etc/mkinitcpio-vm.conf" <<'EOF'
MODULES=(virtio virtio_pci virtio_blk virtio_net virtio_gpu ext4)
BINARIES=()
FILES=()
HOOKS=(base udev modconf block filesystems fsck)
EOF
arch-chroot "$ROOTFS" bash -c 'mkinitcpio -c /etc/mkinitcpio-vm.conf -k "$(ls /usr/lib/modules | head -n1)" -g /boot/initramfs-vm.img' >/dev/null 2>&1
[[ -s $ROOTFS/boot/initramfs-vm.img ]] || { echo "error: VM initramfs was not created"; exit 1; }
rm -f "$IMG"
mke2fs -q -t ext4 -F -L labyrinth -d "$ROOTFS" "$IMG" "$SIZE"
cp "$ROOTFS/boot/initramfs-vm.img" "$OUT/initramfs.img"
ls -la "$OUT" | grep -E "img|vmlinuz"
echo "built. Boot with: scripts/vm/run-vm.sh"
