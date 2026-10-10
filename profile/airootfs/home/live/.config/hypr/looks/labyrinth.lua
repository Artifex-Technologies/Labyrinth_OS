-- The Labyrinth look: macOS-like. Rounded corners, soft shadows and real compositor
-- blur behind translucent windows, tiling by default (SUPER + T floats a window).
--
-- Blur here is Hyprland's own, applied to ANY translucent window (the foot terminal
-- is made translucent in foot/foot.ini). This is why Labyrinth uses Hyprland and not
-- COSMIC for the desktop: COSMIC's blur is opt-in per application. See docs/DESKTOP.md.
--
-- Same constructs as Chymaera's boot-tested modes/stealth.lua; values are a first
-- guess at the look and need tuning on the real GPU.

hl.config({
  general = {
    gaps_in = 6,
    gaps_out = 12,
    border_size = 1,
    col = {
      active_border = "rgba(ffffff33)",
      inactive_border = "rgba(ffffff11)",
    },
    layout = "dwindle",
  },

  decoration = {
    rounding = 16,
    -- 4.0 is a squircle (the Apple-style continuous curve); 2.0 would be a plain circle.
    -- Documented in Hyprland's Variables page; not yet seen running.
    rounding_power = 4.0,
    shadow = {
      enabled = true,
      range = 24,
      render_power = 3,
      color = "rgba(00000055)",
    },
    blur = {
      enabled = true,
      size = 8,
      passes = 3,
    },
  },

  animations = {
    enabled = true,
  },
})

-- The Activity pill's btop window floats (same rule form as Chymaera's tested stealth look).
hl.window_rule({ match = { class = "labyrinth-activity" }, float = true })
