#!/usr/bin/env bash
# Exercise labyrinth-install-cleanup against a mock target root (no root, no
# Linux install needed). Run from the repo root: bash tests/test_install_cleanup.sh
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
script=$here/profile/airootfs/usr/local/bin/labyrinth-install-cleanup
pass=0; failn=0
ok()   { printf 'PASS %s\n' "$1"; pass=$((pass+1)); }
bad()  { printf 'FAIL %s\n' "$1"; failn=$((failn+1)); }
check(){ if eval "$2"; then ok "$1"; else bad "$1"; fi; }

mkroot() {  # $1 = list of kernel pkgbases
  local r; r=$(mktemp -d)
  mkdir -p "$r/usr/share/mkinitcpio" "$r/etc/mkinitcpio.d" "$r/etc/mkinitcpio.conf.d" \
           "$r/etc/sudoers.d" "$r/etc/sddm.conf.d" "$r/home/live/.config/hypr" "$r/boot"
  printf 'ALL_config="/etc/mkinitcpio.conf"\nALL_kver="/boot/vmlinuz-%%PKGBASE%%"\nPRESETS=('"'default'"')\ndefault_image="/boot/initramfs-%%PKGBASE%%.img"\n' >"$r/usr/share/mkinitcpio/hook.preset"
  printf 'archiso preset\n' >"$r/etc/mkinitcpio.d/linux.preset"
  printf 'HOOKS=(archiso)\n' >"$r/etc/mkinitcpio.conf.d/archiso.conf"
  printf 'live ALL=(ALL) NOPASSWD: ALL\n' >"$r/etc/sudoers.d/00-live-nopasswd"
  printf '[Autologin]\nUser=live\n' >"$r/etc/sddm.conf.d/autologin.conf"
  printf 'x' >"$r/home/live/.config/hypr/hyprland.conf"
  for k in $1; do
    mkdir -p "$r/usr/lib/modules/6.99-$k"
    printf '%s\n' "$k" >"$r/usr/lib/modules/6.99-$k/pkgbase"
    printf 'kernel-%s' "$k" >"$r/usr/lib/modules/6.99-$k/vmlinuz"
  done
  echo "$r"
}

# Three kernels: success path.
r=$(mkroot "linux-cachyos linux-cachyos-bore linux-cachyos-lts")
out=$(LABYRINTH_ROOT=$r bash "$script" 2>&1); rc=$?
check "exit 0 with three kernels" "[ $rc -eq 0 ]"
for k in linux-cachyos linux-cachyos-bore linux-cachyos-lts; do
  check "$k kernel copied to /boot"      "[ \"\$(cat $r/boot/vmlinuz-$k)\" = kernel-$k ]"
  check "$k preset names its own files"  "grep -q 'initramfs-$k.img' $r/etc/mkinitcpio.d/$k.preset && ! grep -q '%PKGBASE%' $r/etc/mkinitcpio.d/$k.preset"
done
check "archiso preset removed"           "[ ! -e $r/etc/mkinitcpio.d/linux.preset ]"
check "archiso initcpio drop-in removed" "[ ! -e $r/etc/mkinitcpio.conf.d/archiso.conf ]"
check "live sudoers removed"             "[ ! -e $r/etc/sudoers.d/00-live-nopasswd ]"
check "autologin removed"                "[ ! -e $r/etc/sddm.conf.d/autologin.conf ]"
check "live home removed"                "[ ! -d $r/home/live ]"
check "desktop config seeded to skel"    "[ -f $r/etc/skel/.config/hypr/hyprland.conf ]"
check "no LTS warning when LTS present"  "! grep -q WARNING <<<\"\$out\""
rm -rf "$r"

# Missing LTS: warns but succeeds.
r=$(mkroot "linux-cachyos")
out=$(LABYRINTH_ROOT=$r bash "$script" 2>&1); rc=$?
check "single kernel still succeeds" "[ $rc -eq 0 ]"
check "warns when LTS fallback missing" "grep -q 'no fallback kernel' <<<\"\$out\""
rm -rf "$r"

# No kernel: must fail.
r=$(mkroot "")
LABYRINTH_ROOT=$r bash "$script" >/dev/null 2>&1; rc=$?
check "fails with no kernel" "[ $rc -ne 0 ]"
rm -rf "$r"

# Empty template: must fail.
r=$(mkroot "linux-cachyos"); : >"$r/usr/share/mkinitcpio/hook.preset"
LABYRINTH_ROOT=$r bash "$script" >/dev/null 2>&1; rc=$?
check "fails with empty preset template" "[ $rc -ne 0 ]"
rm -rf "$r"

# Template without PRESETS: must fail rather than ship an unbootable preset.
r=$(mkroot "linux-cachyos"); printf 'nothing useful\n' >"$r/usr/share/mkinitcpio/hook.preset"
LABYRINTH_ROOT=$r bash "$script" >/dev/null 2>&1; rc=$?
check "fails when generated preset has no PRESETS" "[ $rc -ne 0 ]"
rm -rf "$r"

# Hostile pkgbase must be rejected (it is interpolated into sed and file names).
r=$(mkroot ""); mkdir -p "$r/usr/lib/modules/x"; printf 'a|b;c\n' >"$r/usr/lib/modules/x/pkgbase"; printf k >"$r/usr/lib/modules/x/vmlinuz"
LABYRINTH_ROOT=$r bash "$script" >/dev/null 2>&1; rc=$?
check "rejects odd pkgbase" "[ $rc -ne 0 ]"
rm -rf "$r"

printf '\n%d passed, %d failed\n' "$pass" "$failn"
[ "$failn" -eq 0 ]
