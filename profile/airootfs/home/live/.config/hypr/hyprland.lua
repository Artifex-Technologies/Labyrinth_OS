-- Labyrinth Hyprland session.
--
-- Structure and key layout follow Omarchy (MIT, Copyright (c) David Heinemeier
-- Hansson; see THIRD_PARTY_NOTICES.md) via Chymaera_OS, whose session of this shape
-- was boot-tested in a VM. Config is Lua because Hyprland deprecated hyprlang in
-- 0.55. Only constructs already used by Chymaera's tested config appear here; the
-- Lua itself has NOT been parsed or run for Labyrinth. See docs/DESKTOP.md.

local home = os.getenv("HOME")
local config_dir = home .. "/.config/hypr"

-- Same shape as Omarchy's o.bind: a string dispatcher runs as a command.
o = o or {}
function o.bind(keys, description, dispatcher, options)
  local opts = options or {}
  if description then
    opts.description = description
  end
  if type(dispatcher) == "string" then
    dispatcher = hl.dsp.exec_cmd(dispatcher)
  end
  hl.bind(keys, dispatcher, opts)
end

-- Environment. Wayland everywhere. The NVIDIA lines are for the RTX 2080 Ti.
hl.env("XCURSOR_SIZE", "24")
hl.env("GDK_BACKEND", "wayland,x11,*")
hl.env("QT_QPA_PLATFORM", "wayland;xcb")
hl.env("QT_STYLE_OVERRIDE", "kvantum")
hl.env("MOZ_ENABLE_WAYLAND", "1")
hl.env("ELECTRON_OZONE_PLATFORM_HINT", "wayland")
hl.env("XDG_SESSION_TYPE", "wayland")
hl.env("XDG_CURRENT_DESKTOP", "Hyprland")
hl.env("XDG_SESSION_DESKTOP", "Hyprland")
hl.env("LIBVA_DRIVER_NAME", "nvidia")
hl.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia")
hl.env("NVD_BACKEND", "direct")

hl.config({
  input = {
    kb_layout = "us",
    -- 2 = click to focus (cursor movement alone does not change focus), as on macOS.
    follow_mouse = 2,
  },

  general = {
    layout = "dwindle",
    allow_tearing = false,
    -- Drag a window's edge or corner to resize it (there are no title bars to grab).
    resize_on_border = true,
    extend_border_grab_area = 12,
  },

  dwindle = {
    preserve_split = true,
    force_split = 2,
  },

  misc = {
    disable_hyprland_logo = true,
    disable_splash_rendering = true,
    focus_on_activate = true,
  },

  cursor = {
    hide_on_key_press = true,
  },
})

-- Windows behave like macOS: every window floats, centred, at the size the app asks for, and you
-- move and resize it freely. Tiling is opt-in per window with SUPER + T (see bindings/tiling.lua).
hl.window_rule({
  name = "windows-float-like-macos",
  match = { class = ".*" },
  float = true,
  center = true,
})

-- Applications and shell.
o.bind("SUPER + RETURN", "Terminal", "uwsm-app -- foot")
o.bind("SUPER + SHIFT + RETURN", "Browser", "uwsm-app -- vivaldi")
o.bind("SUPER + SHIFT + B", "Browser", "uwsm-app -- vivaldi")
o.bind("SUPER + SHIFT + ALT + B", "Backup browser (Firefox)", "uwsm-app -- firefox")
o.bind("SUPER + SHIFT + F", "File manager", "uwsm-app -- finder")
o.bind("SUPER + SHIFT + N", "Notes", "uwsm-app -- obsidian")
o.bind("SUPER + SPACE", "Launcher", "fuzzel")
o.bind("SUPER + ESCAPE", "Power and quick settings", "labyrinth-quick-settings")
o.bind("SUPER + CTRL + L", "Lock screen", "pidof hyprlock || hyprlock")
o.bind("SUPER + K", "Show keybindings", "labyrinth-keys")
o.bind("SUPER + CTRL + N", "New desktop", "labyrinth-workspace new")

-- The Windows shortcuts, so muscle memory from Windows works here too.
o.bind("SUPER + E", "File manager (Windows: Explorer)", "uwsm-app -- finder")
o.bind("SUPER + R", "Run (Windows: Run box)", "fuzzel")
o.bind("SUPER + I", "Settings", "uwsm-app -- systemsettings")
o.bind("SUPER + L", "Lock screen", "pidof hyprlock || hyprlock")
o.bind("SUPER + V", "Clipboard history", "cliphist list | fuzzel --dmenu | cliphist decode | wl-copy")
o.bind("SUPER + SHIFT + S", "Screenshot region", "grim -g \"$(slurp)\" - | wl-copy")
o.bind("CTRL + SHIFT + ESCAPE", "Task manager", "uwsm-app -- plasma-systemmonitor")

-- Screenshots, clipboard, media. Spotify is the default media target.
o.bind("PRINT", "Screenshot region", "grim -g \"$(slurp)\" - | wl-copy")
o.bind("SUPER + CTRL + V", "Clipboard history", "cliphist list | fuzzel --dmenu | cliphist decode | wl-copy")
o.bind("XF86AudioRaiseVolume", "Volume up", "wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+", { locked = true, repeating = true })
o.bind("XF86AudioLowerVolume", "Volume down", "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-", { locked = true, repeating = true })
o.bind("XF86AudioMute", "Mute", "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle", { locked = true })
o.bind("XF86AudioPlay", "Play / pause", "playerctl play-pause", { locked = true })
o.bind("XF86AudioNext", "Next track", "playerctl next", { locked = true })
o.bind("XF86AudioPrev", "Previous track", "playerctl previous", { locked = true })

dofile(config_dir .. "/bindings/tiling.lua")

-- Session services. The bar and dock are started by labyrinth-shell.
hl.on("hyprland.start", function()
  hl.exec_cmd("systemctl --user import-environment $(env | cut -d'=' -f 1)")
  hl.exec_cmd("dbus-update-activation-environment --systemd --all")
  hl.exec_cmd("uwsm-app -- hyprpolkitagent")
  hl.exec_cmd("uwsm-app -- mako")
  hl.exec_cmd("uwsm-app -- hypridle")
  hl.exec_cmd("wl-paste --watch cliphist store")
  hl.exec_cmd("labyrinth-displays --apply")
  hl.exec_cmd("labyrinth-shell apply")
end)

-- Monitor layout written by labyrinth-displays at login (see docs/DESKTOP.md). Loaded in a
-- protected call so a missing or bad file can never stop the session starting.
local monitors_file = io.open(config_dir .. "/monitors.lua", "r")
if monitors_file then
  monitors_file:close()
  local ok, err = pcall(dofile, config_dir .. "/monitors.lua")
  if not ok then
    hl.exec_cmd("notify-send 'Labyrinth' 'monitors.lua failed to load; using defaults'")
  end
end

dofile(config_dir .. "/looks/labyrinth.lua")
