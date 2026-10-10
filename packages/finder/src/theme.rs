//! Colors, after EdenExplorer's dark look: a near-black blue-grey, a purple accent for the active
//! tab and selection, and green / amber / red drive-usage bars. The light palette is a plain
//! counterpart, not a design of its own.

use eframe::egui::{self, Color32, CornerRadius};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub strip: Color32,
    pub field: Color32,
    pub border: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub tab_active: Color32,
    pub hover: Color32,
    pub select: Color32,
    pub row_alt: Color32,
    pub folder: Color32,
    pub file: Color32,
    pub drive: Color32,
    pub green: Color32,
    pub amber: Color32,
    pub red: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: Color32::from_rgb(16, 17, 22),
            sidebar: Color32::from_rgb(14, 15, 20),
            strip: Color32::from_rgb(11, 12, 16),
            field: Color32::from_rgb(24, 25, 32),
            border: Color32::from_rgb(38, 40, 52),
            text: Color32::from_rgb(232, 233, 238),
            dim: Color32::from_rgb(140, 143, 157),
            accent: Color32::from_rgb(124, 84, 214),
            tab_active: Color32::from_rgb(72, 44, 128),
            hover: Color32::from_rgb(32, 34, 44),
            select: Color32::from_rgb(52, 40, 92),
            row_alt: Color32::from_rgb(19, 20, 26),
            folder: Color32::from_rgb(240, 196, 64),
            file: Color32::from_rgb(190, 194, 206),
            drive: Color32::from_rgb(176, 180, 194),
            green: Color32::from_rgb(58, 158, 94),
            amber: Color32::from_rgb(201, 146, 58),
            red: Color32::from_rgb(180, 58, 69),
        }
    } else {
        Palette {
            bg: Color32::from_rgb(250, 250, 252),
            sidebar: Color32::from_rgb(243, 243, 247),
            strip: Color32::from_rgb(232, 232, 238),
            field: Color32::from_rgb(255, 255, 255),
            border: Color32::from_rgb(214, 214, 224),
            text: Color32::from_rgb(28, 29, 36),
            dim: Color32::from_rgb(110, 113, 128),
            accent: Color32::from_rgb(110, 70, 200),
            tab_active: Color32::from_rgb(196, 176, 238),
            hover: Color32::from_rgb(232, 232, 240),
            select: Color32::from_rgb(214, 200, 244),
            row_alt: Color32::from_rgb(244, 244, 248),
            folder: Color32::from_rgb(232, 178, 40),
            file: Color32::from_rgb(120, 126, 142),
            drive: Color32::from_rgb(110, 116, 132),
            green: Color32::from_rgb(46, 140, 80),
            amber: Color32::from_rgb(196, 130, 30),
            red: Color32::from_rgb(190, 50, 60),
        }
    }
}

/// Usage bar color: green below 75%, amber below 95%, red above (Eden's 68% green, 92% amber, 100% red).
pub fn usage_color(p: &Palette, fraction: f32) -> Color32 {
    if fraction < 0.75 {
        p.green
    } else if fraction < 0.95 {
        p.amber
    } else {
        p.red
    }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.panel_fill = p.bg;
        v.window_fill = p.field;
        v.extreme_bg_color = p.field;
        v.faint_bg_color = p.row_alt;
        v.window_stroke = egui::Stroke::new(1.0, p.border);
        v.selection.bg_fill = p.select;
        v.selection.stroke = egui::Stroke::new(1.0, p.accent);
        v.override_text_color = Some(p.text);
        let r = CornerRadius::same(7);
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = r;
        }
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
        v.widgets.inactive.weak_bg_fill = p.field;
        v.widgets.inactive.bg_fill = p.field;
        v.widgets.hovered.weak_bg_fill = p.hover;
        v.widgets.hovered.bg_fill = p.hover;
        v.widgets.active.weak_bg_fill = p.select;
        v.widgets.active.bg_fill = p.select;
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    });
}
