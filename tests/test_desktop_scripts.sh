#!/usr/bin/env bash
# Checks on the live user's desktop config files (seeded into /etc/skel for new users). No desktop needed. Run from the
# repo root. (The old tiling desktop's quick-settings menu and bar were removed on 2026-11-05; Yggdrasil is the desktop.)
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
cfg=$here/profile/airootfs/home/live/.config
pass=0; failn=0
ok()   { printf 'PASS %s\n' "$1"; pass=$((pass+1)); }
bad()  { printf 'FAIL %s\n' "$1"; failn=$((failn+1)); }
check(){ if eval "$2"; then ok "$1"; else bad "$1"; fi; }

check "gtk: buttons on the right" "grep -q ':minimize,maximize,close' $cfg/gtk-3.0/settings.ini && grep -q ':minimize,maximize,close' $cfg/gtk-4.0/settings.ini"
check "no config for the removed tiling desktop" "[ ! -e $cfg/waybar ] && [ ! -e $cfg/hypr ] && [ ! -e $cfg/mako ] && [ ! -e $cfg/fuzzel ] && [ ! -e $cfg/foot ]"

printf '\n%d passed, %d failed\n' "$pass" "$failn"
[ "$failn" -eq 0 ]
