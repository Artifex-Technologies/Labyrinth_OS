#!/usr/bin/env bash
# Boot the built ISO in QEMU with UEFI (OVMF) and KVM, with a blank target disk to install onto.
#   scripts/vm/run-iso.sh iso        boot the ISO (live session) with the blank disk attached
#   scripts/vm/run-iso.sh disk       boot from the target disk only (after an install)
#   scripts/vm/run-iso.sh new        delete and recreate the blank disk and UEFI variables
# Headless. Screenshots and input: scripts/vm/qmp-shot.py (socket $OUT/qmp.sock).
# Serial console (a root shell on the live ISO): socket $OUT/ser.sock, driven by scripts/vm/serial.py.
# Env: ACCEL (default -enable-kvm; "-accel tcg" for pure emulation), CPU (QEMU cpu model, default host), VGA_ARGS (display adapter args, default virtio-vga; try "-vga std"), ISO (default /root/isoout/labyrinth-*.iso), OUT (/root/vmiso), DISK_GB (80), RAM (6G).
set -eu
OUT=${OUT:-/root/vmiso}
MODE=${1:-iso}
DISK_GB=${DISK_GB:-80}
RAM=${RAM:-6G}
RES=${RES:-1720x720}
mkdir -p "$OUT"
FW=/usr/share/edk2/x64
if [[ $MODE == new || ! -f $OUT/disk.qcow2 ]]; then
  rm -f "$OUT/disk.qcow2" "$OUT/OVMF_VARS.fd"
  qemu-img create -q -f qcow2 "$OUT/disk.qcow2" "${DISK_GB}G"
  cp "$FW/OVMF_VARS.4m.fd" "$OUT/OVMF_VARS.fd"
  [[ $MODE == new ]] && { echo "fresh disk and UEFI variables in $OUT"; exit 0; }
fi
rm -f "$OUT/qmp.sock" "$OUT/ser.sock"
ARGS=(${ACCEL:--enable-kvm} -cpu "${CPU:-host}" -smp 6 -m "$RAM" -machine q35
  -drive "if=pflash,format=raw,readonly=on,file=$FW/OVMF_CODE.4m.fd"
  -drive "if=pflash,format=raw,file=$OUT/OVMF_VARS.fd"
  -drive "file=$OUT/disk.qcow2,if=virtio,format=qcow2,cache=unsafe"
  ${VGA_ARGS:--device virtio-vga,xres=${RES%x*},yres=${RES#*x}}
  -device qemu-xhci -device usb-kbd -device usb-tablet
  -nic user,model=virtio-net-pci
  -display none -qmp "unix:$OUT/qmp.sock,server,nowait"
  -serial "unix:$OUT/ser.sock,server,nowait"
  -name labyrinth-iso)
if [[ $MODE == iso ]]; then
  ISO=${ISO:-$(ls -t /root/isoout/labyrinth-*.iso | head -n1)}
  ARGS+=(-drive "file=$ISO,media=cdrom,if=none,id=cd0,readonly=on" -device ide-cd,drive=cd0,bootindex=0)
fi
exec qemu-system-x86_64 "${ARGS[@]}"
