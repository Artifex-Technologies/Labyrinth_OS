#!/usr/bin/env bash
# Boot the image made by build-vm-image.sh in QEMU with KVM.
#   scripts/vm/run-vm.sh            headless: no window; use qmp-shot.py to take screenshots
#   scripts/vm/run-vm.sh window     open a window (WSLg) you can use with mouse and keyboard
# Log in is automatic (user "labyrinth", password "labyrinth", also root).
set -eu
OUT=${OUT:-/root/vm}
MODE=${1:-headless}
RES=${RES:-1720x720}
case $MODE in
  window)   DISP="gtk,gl=off,grab-on-hover=on" ;;
  headless) DISP="none" ;;
  *) echo "usage: $0 [headless|window]"; exit 2 ;;
esac
rm -f "$OUT/qmp.sock" "$OUT/serial.log"
ARGS=(-enable-kvm -cpu host -smp 4 -m 4G
  -kernel "$OUT/vmlinuz-linux" -initrd "$OUT/initramfs.img"
  -append "root=/dev/vda rw rootfstype=ext4 console=ttyS0 loglevel=4 video=Virtual-1:${RES}"
  -drive "file=$OUT/labyrinth.img,if=virtio,format=raw"
  -device "virtio-vga,xres=${RES%x*},yres=${RES#*x}"
  -device qemu-xhci -device usb-kbd -device usb-tablet
  -nic user,model=virtio-net-pci
  -display "$DISP" -qmp "unix:$OUT/qmp.sock,server,nowait" -serial "file:$OUT/serial.log"
  -name labyrinth-desktop)

if [[ $MODE == headless ]]; then
  exec qemu-system-x86_64 "${ARGS[@]}"
fi

# Window mode. QEMU's GTK window tells the guest its own size (640x480 at first), which overrides the
# size asked for above. Run GTK on X11 so the window can be resized from outside, then resize it to
# ${RES}; the guest follows and Hyprland re-lays-out the bar.
export GDK_BACKEND=x11
qemu-system-x86_64 "${ARGS[@]}" &
QPID=$!
(
  for _ in $(seq 1 60); do
    WID=$(xdotool search --pid "$QPID" 2>/dev/null | tail -n 1)
    [[ -n ${WID:-} ]] && { sleep 1; xdotool windowsize "$WID" "${RES%x*}" "${RES#*x}"; break; }
    sleep 0.5
  done
) &
wait "$QPID"
