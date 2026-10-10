#!/usr/bin/env bash
# End-to-end test of finder: drives the REAL window with xdotool and checks the
# REAL filesystem afterwards. Needs a graphical session (WSLg, or X11/XWayland) and:
#   pacman -S xdotool xorg-xwininfo mesa vulkan-swrast
# Usage: tests/gui_smoke_wsl.sh [path-to-finder-binary]
#
# Coordinates assume the default 1200x760 window, Details view, hidden files off, and a
# demo folder containing Music/, Photos/, notes.md, "Report Final.txt" in that order
# (rows at y = 138, 163, 188, 213). If the toolbar layout changes, adjust them.
# Software rendering is forced so it also works without a GPU (WSL).
set -u
BIN=${1:-./target/debug/finder}
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/mnt/wslg/runtime-dir} DISPLAY=${DISPLAY:-:0} WAYLAND_DISPLAY= \
       LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=gl
# Lowercase + digits only: xdotool typing of mixed-case paths into the address bar was unreliable.
DEMO=/tmp/lfdemo$$; mkdir -p "$DEMO"
HOME_SANDBOX=/tmp/lfhome$$; mkdir -p "$HOME_SANDBOX"   # keeps the app's state and Trash out of the real home
export HOME=$HOME_SANDBOX XDG_CONFIG_HOME=$HOME_SANDBOX/.config XDG_DATA_HOME=$HOME_SANDBOX/.local/share

pass=0; failn=0
check() { if eval "$2"; then echo "PASS $1"; pass=$((pass+1)); else echo "FAIL $1"; failn=$((failn+1)); fi; }

mkdir -p "$DEMO/Music" "$DEMO/Photos"; echo a > "$DEMO/notes.md"; echo b > "$DEMO/Report Final.txt"
pkill -x finder 2>/dev/null; sleep 1   # no stale instances
"$BIN" > /tmp/lf-gui.out 2>&1 &
APP=$!
trap 'kill $APP 2>/dev/null; rm -rf "$DEMO" "$HOME_SANDBOX"' EXIT
# Find OUR window by process id, never by title alone: a leftover instance from an earlier run
# has the same title and would silently receive the clicks.
for _ in $(seq 1 30); do WID=$(xdotool search --pid "$APP" --name "Finder" 2>/dev/null | head -1); [ -n "${WID:-}" ] && break; sleep 0.5; done
[ -z "${WID:-}" ] && { echo "FAIL window never appeared"; cat /tmp/lf-gui.out; exit 1; }
sleep 6; xdotool windowfocus "$WID"; sleep 0.5   # software-rendered first frames are slow
click() { xdotool mousemove --window "$WID" "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.7; }

# go to the demo folder through the address bar
click 500 61; xdotool key ctrl+a; xdotool type --delay 40 "$DEMO"; xdotool key Return; sleep 1.5

click 279 188; xdotool key F2; sleep 1.2; xdotool key ctrl+a; sleep 0.3; xdotool type --delay 50 "ideas.md"; sleep 0.5; xdotool key Return; sleep 1.5
check "rename (F2, type, Enter)" "[ -f '$DEMO/ideas.md' ] && [ ! -e '$DEMO/notes.md' ]"

click 297 213; xdotool key ctrl+c; sleep 0.6; xdotool key ctrl+v; sleep 1.5
check "copy + paste creates a numbered copy" "[ -f '$DEMO/Report Final (2).txt' ] && [ -f '$DEMO/Report Final.txt' ]"

click 279 188; xdotool key ctrl+x; sleep 0.6
xdotool mousemove --window "$WID" 272 163; sleep 0.3; xdotool click --repeat 2 --delay 120 1; sleep 1.2
xdotool key ctrl+v; sleep 1.5
check "cut + paste moves into another folder" "[ -f '$DEMO/Photos/ideas.md' ] && [ ! -e '$DEMO/ideas.md' ]"

xdotool key ctrl+shift+n; sleep 1.2; xdotool key Return; sleep 1.5
check "new folder (Ctrl+Shift+N)" "[ -d '$DEMO/Photos/New folder' ]"

xdotool key BackSpace; sleep 1.2      # up to the demo folder
click 272 138; xdotool key Delete; sleep 1.5
check "delete goes to the freedesktop Trash, with its origin recorded" "[ ! -e '$DEMO/Music' ] && [ -d '$XDG_DATA_HOME/Trash/files/Music' ] && grep -q 'Path=.*Music' '$XDG_DATA_HOME/Trash/info/Music.trashinfo'"

sleep 1.5
check "session state is saved" "[ -s '$XDG_CONFIG_HOME/finder/state.json' ]"
check "the app is still running (no crash)" "kill -0 $APP 2>/dev/null"

if [ "$failn" -ne 0 ]; then
  echo "--- demo folder now:"; ls -RA "$DEMO" 2>&1 | head -20
  echo "--- tab paths the app saved:"; python3 -c "import json; print([t['path'] for t in json.load(open('$XDG_CONFIG_HOME/finder/state.json'))['tabs']])" 2>&1 | tail -1
  echo "--- app output:"; head -5 /tmp/lf-gui.out
  echo "--- (if every step failed the window probably had no keyboard focus: run via tests/gui_smoke_wsl.ps1)"
fi
printf '\n%d passed, %d failed\n' "$pass" "$failn"
[ "$failn" -eq 0 ]
