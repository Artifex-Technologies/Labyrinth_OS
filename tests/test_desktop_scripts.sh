#!/usr/bin/env bash
# Tests for labyrinth-quick-settings using stand-ins for curl,
# the menu, and systemctl. No desktop needed. Run from the repo root.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
bin=$here/profile/airootfs/usr/local/bin
cfg=$here/profile/airootfs/home/live/.config
pass=0; failn=0
ok()   { printf 'PASS %s\n' "$1"; pass=$((pass+1)); }
bad()  { printf 'FAIL %s\n' "$1"; failn=$((failn+1)); }
check(){ if eval "$2"; then ok "$1"; else bad "$1"; fi; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"

# ---- quick settings ------------------------------------------------------------
cat >"$tmp/bin/menu" <<'EOF'
#!/usr/bin/env bash
# Stand-in for `fuzzel --dmenu`: log what was offered, answer with the next scripted choice.
cat >>"$MENU_LOG"; echo "---" >>"$MENU_LOG"
c=$(head -n1 "$CHOICES"); sed -i 1d "$CHOICES"
[ -z "$c" ] && exit 1
printf '%s\n' "$c"
EOF
for cmd in systemctl uwsm hyprctl; do
  printf '#!/usr/bin/env bash\necho "%s $*" >>"$ACT_LOG"\n' "$cmd" >"$tmp/bin/$cmd"
done
chmod +x "$tmp/bin/"*
export MENU_LOG=$tmp/menu.log ACT_LOG=$tmp/act.log CHOICES=$tmp/choices
export LABYRINTH_MENU="$tmp/bin/menu"

printf 'MemTotal:       65536000 kB\n' >"$tmp/meminfo"
printf 'Filename Type Size Used Priority\n/dev/zram0 partition 8000000 0 100\n' >"$tmp/swaps_zram"
printf 'Filename Type Size Used Priority\n/swap/swapfile file 70000000 0 -2\n' >"$tmp/swaps_real"
printf 'freeze mem disk\n' >"$tmp/state_yes"; printf 'freeze mem\n' >"$tmp/state_no"
printf 'root=x resume=UUID=abc quiet\n' >"$tmp/cmd_resume"; printf 'root=x quiet\n' >"$tmp/cmd_plain"
export LABYRINTH_PROC_MEMINFO=$tmp/meminfo

qs() {  # qs <swaps> <state> <cmdline> <choices...>
  : >"$MENU_LOG"; : >"$ACT_LOG"
  export LABYRINTH_PROC_SWAPS=$tmp/$1 LABYRINTH_SYS_POWER_STATE=$tmp/$2 LABYRINTH_PROC_CMDLINE=$tmp/$3
  shift 3; printf '%s\n' "$@" >"$CHOICES"
  PATH="$tmp/bin:$PATH" bash "$bin/labyrinth-quick-settings"
}

qs swaps_zram state_yes cmd_resume "Sleep"
check "qs: default (zram swap) does not offer Hibernate" "! grep -q Hibernate $MENU_LOG"
check "qs: Sleep suspends" "grep -q 'systemctl suspend' $ACT_LOG"
qs swaps_real state_yes cmd_resume "Hibernate"
check "qs: real swap >= RAM + resume= offers and runs Hibernate" "grep -q Hibernate $MENU_LOG && grep -q 'systemctl hibernate' $ACT_LOG"
qs swaps_real state_no cmd_resume "Sleep"
check "qs: kernel without disk state hides Hibernate" "! grep -q Hibernate $MENU_LOG"
qs swaps_real state_yes cmd_plain "Sleep"
check "qs: no resume= hides Hibernate" "! grep -q Hibernate $MENU_LOG"
qs swaps_zram state_yes cmd_resume "Power off" "Yes, power off"
check "qs: Power off confirmed -> poweroff" "grep -q 'systemctl poweroff' $ACT_LOG"
qs swaps_zram state_yes cmd_resume "Power off" "Cancel"
check "qs: Power off cancelled -> nothing runs" "[ ! -s $ACT_LOG ]"
qs swaps_zram state_yes cmd_resume "Power off"
check "qs: dismissing the confirmation does not power off" "[ ! -s $ACT_LOG ]"
qs swaps_zram state_yes cmd_resume "Restart" "Yes, restart"
check "qs: Restart confirmed -> reboot" "grep -q 'systemctl reboot' $ACT_LOG"
qs swaps_zram state_yes cmd_resume "Log out" "Yes, log out"
check "qs: Log out uses uwsm stop" "grep -q 'uwsm stop' $ACT_LOG"
qs swaps_zram state_yes cmd_resume
check "qs: dismissing the menu does nothing" "[ ! -s $ACT_LOG ]"
check "qs: menu always lists the six actions' basics" "grep -q 'Settings' $MENU_LOG && grep -q 'Log out' $MENU_LOG && grep -q 'Restart' $MENU_LOG"

# ---- config files ----------------------------------------------------------------
python3 - "$cfg/waybar/config.jsonc" <<'EOF' 2>/dev/null || python - "$cfg/waybar/config.jsonc" <<'EOF2'
import json, re, sys
t = open(sys.argv[1], encoding="utf-8").read()
t = re.sub(r'^\s*//.*$', '', t, flags=re.M)
d = json.loads(t)
mods = d["modules-left"] + d["modules-right"]
missing = [m for m in mods if m not in d]
sys.exit(1 if missing else 0)
EOF
import json, re, sys
t = open(sys.argv[1], encoding="utf-8").read()
t = re.sub(r'^\s*//.*$', '', t, flags=re.M)
d = json.loads(t)
mods = d["modules-left"] + d["modules-right"]
missing = [m for m in mods if m not in d]
sys.exit(1 if missing else 0)
EOF2
[ $? -eq 0 ] && ok "waybar config parses and defines every listed module" || bad "waybar config parses and defines every listed module"
check "waybar clock has no GNU dash flags (comments ignored)" "! grep -v '^[[:space:]]*//' $cfg/waybar/config.jsonc | grep -q '%-'"
check "gtk: buttons on the right" "grep -q ':minimize,maximize,close' $cfg/gtk-3.0/settings.ini && grep -q ':minimize,maximize,close' $cfg/gtk-4.0/settings.ini"
check "shell scripts have valid syntax" "bash -n $bin/labyrinth-shell && bash -n $bin/labyrinth-quick-settings"

printf '\n%d passed, %d failed\n' "$pass" "$failn"
[ "$failn" -eq 0 ]
