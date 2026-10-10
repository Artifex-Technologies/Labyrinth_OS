#!/usr/bin/env bash
# Run and drive a GUI app on a VIRTUAL display (Xvfb): no window ever appears on the real desktop,
# and keyboard focus is not an issue (unlike WSLg), so synthetic input is reliable.
#
#   scripts/xvfb/xv.sh start CMD [ARGS...]    start Xvfb (:99) and CMD on it (software rendering)
#   scripts/xvfb/xv.sh shot OUT.png           screenshot the whole virtual screen
#   scripts/xvfb/xv.sh do XDOTOOL-ARGS...     e.g.  do mousemove 300 200 click 1   |  do key ctrl+t
#   scripts/xvfb/xv.sh type "text"            type text into the focused widget
#   scripts/xvfb/xv.sh stop
#
# Needs: xorg-server-xvfb xdotool imagemagick (Arch).  Screen size: XV_SIZE (default 1400x860x24).
set -u
export DISPLAY=:99
# Never let the app reach the real desktop: with WAYLAND_DISPLAY set (WSLg) it would open a visible
# window there instead of on the virtual display.
unset WAYLAND_DISPLAY XDG_SESSION_TYPE
export WINIT_UNIX_BACKEND=x11 GDK_BACKEND=x11
SIZE=${XV_SIZE:-1400x860x24}
STATE=${XV_STATE:-/tmp/xv}
mkdir -p "$STATE"

case "${1:-}" in
  start)
    shift
    "$0" stop >/dev/null 2>&1
    # setsid + nohup: survive the end of the shell session that started them (WSL kills the rest).
    setsid nohup Xvfb :99 -screen 0 "$SIZE" -nolisten tcp >"$STATE/xvfb.log" 2>&1 < /dev/null &
    echo $! >"$STATE/xvfb.pid"
    for _ in $(seq 1 30); do xdotool getdisplaygeometry >/dev/null 2>&1 && break; sleep 0.2; done
    LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=gl setsid nohup "$@" >"$STATE/app.log" 2>&1 < /dev/null &
    echo $! >"$STATE/app.pid"
    sleep "${XV_WAIT:-6}"
    kill -0 "$(cat "$STATE/app.pid")" 2>/dev/null && echo "app running" || { echo "app exited:"; head -5 "$STATE/app.log"; exit 1; }
    ;;
  shot)
    import -window root "$2" && echo "saved $2"
    ;;
  do)
    shift
    xdotool "$@"
    sleep 0.6
    ;;
  type)
    xdotool type --delay 30 "$2"
    sleep 0.4
    ;;
  stop)
    for f in app xvfb; do [ -f "$STATE/$f.pid" ] && kill "$(cat "$STATE/$f.pid")" 2>/dev/null; rm -f "$STATE/$f.pid"; done
    echo stopped
    ;;
  *)
    sed -n '2,13p' "$0"; exit 2 ;;
esac
