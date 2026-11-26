#!/usr/bin/env bash
# The start-up image hook labyrinth-gpu-firmware (profile/airootfs/usr/lib/initcpio/install): an empty placeholder in the
# image for each NVIDIA open-driver firmware file of a card generation the computer does not have, nothing else.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
pass=0; fail=0
check() { if eval "$2"; then echo "PASS $1"; pass=$((pass + 1)); else echo "FAIL $1"; fail=$((fail + 1)); fi; }

run() {  # $1.. = device ids of NVIDIA display controllers
  local t i=0; t=$(mktemp -d)
  mkdir -p "$t/fw/nvidia/620.73.04" "$t/root"
  for f in gsp_tu10x gsp_ga10x ucodes_tu10x ucodes_ga10x; do printf 'firmware' >"$t/fw/nvidia/620.73.04/$f.bin"; done
  mkdir -p "$t/sys/bus/pci/devices/0000:00:02.0"
  printf '0x8086\n' >"$t/sys/bus/pci/devices/0000:00:02.0/vendor"; printf '0x030000\n' >"$t/sys/bus/pci/devices/0000:00:02.0/class"
  printf '0xa7a0\n' >"$t/sys/bus/pci/devices/0000:00:02.0/device"
  for id in "$@"; do
    i=$((i + 1)); mkdir -p "$t/sys/bus/pci/devices/0000:0$i:00.0"
    printf '0x10de\n' >"$t/sys/bus/pci/devices/0000:0$i:00.0/vendor"; printf '0x030000\n' >"$t/sys/bus/pci/devices/0000:0$i:00.0/class"
    printf '%s\n' "$id" >"$t/sys/bus/pci/devices/0000:0$i:00.0/device"
  done
  ( BUILDROOT=$t/root LABYRINTH_HW_SYS=$t/sys LABYRINTH_FW_DIR=$t/fw
    # shellcheck source=/dev/null
    . "$here/profile/airootfs/usr/lib/initcpio/install/labyrinth-gpu-firmware"; build )
  echo "$t"
}

held() { find "$1/root" -type f -printf '%f %s\n' | sort | tr '\n' ' '; }

t=$(run 0x1e07)
check "a Turing card: Ampere's two files held out" "[[ \"\$(held $t)\" == 'gsp_ga10x.bin 0 ucodes_ga10x.bin 0 ' ]]"
rm -rf "$t"
t=$(run 0x2684)
check "an Ada card: Turing's two files held out" "[[ \"\$(held $t)\" == 'gsp_tu10x.bin 0 ucodes_tu10x.bin 0 ' ]]"
rm -rf "$t"
t=$(run 0x20b0)
check "GA100 uses Turing's firmware" "[[ \"\$(held $t)\" == 'gsp_ga10x.bin 0 ucodes_ga10x.bin 0 ' ]]"
rm -rf "$t"
t=$(run 0x1e07 0x2204)
check "both generations: nothing held out" "[[ -z \"\$(held $t)\" ]]"
rm -rf "$t"
t=$(run 0x1b80)
check "a card older than Turing: nothing held out" "[[ -z \"\$(held $t)\" ]]"
rm -rf "$t"
t=$(run)
check "no NVIDIA card: nothing held out" "[[ -z \"\$(held $t)\" ]]"
rm -rf "$t"
check "the drop-in leaves the live medium's image alone" "(HOOKS=(base archiso); . '$here/profile/airootfs/etc/mkinitcpio.conf.d/labyrinth-gpu-firmware.conf'; [[ \" \${HOOKS[*]} \" != *labyrinth-gpu-firmware* ]])"
check "the drop-in adds the hook once" "(HOOKS=(base kms); . '$here/profile/airootfs/etc/mkinitcpio.conf.d/labyrinth-gpu-firmware.conf'; . '$here/profile/airootfs/etc/mkinitcpio.conf.d/labyrinth-gpu-firmware.conf'; [[ \"\${HOOKS[*]}\" == 'base kms labyrinth-gpu-firmware' ]])"
echo; echo "$pass passed, $fail failed"
((fail == 0))
