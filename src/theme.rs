//! Look and feel: palettes, egui visuals, fonts.
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, Shadow, Stroke};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,       // window background
    pub panel: Color32,    // side panels / cards
    pub toolbar: Color32,  // toolbars
    pub editor: Color32,   // editor + grid background
    pub border: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub zebra: Color32,
    pub hover: Color32,
    pub grid_line: Color32,
    pub cell_mod: Color32,
    pub cell_new: Color32,
    pub cell_del: Color32,
    pub header: Color32,
    pub number: Color32,
}

pub fn pal(dark: bool) -> Palette {
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    let a = |r, g, b, a| Color32::from_rgba_unmultiplied(r, g, b, a);
    if dark {
        Palette {
            bg: c(30, 33, 39),
            panel: c(37, 41, 49),
            toolbar: c(43, 48, 58),
            editor: c(26, 29, 34),
            border: c(58, 64, 76),
            text: c(215, 218, 224),
            weak: c(133, 140, 153),
            accent: c(91, 156, 255),
            accent_soft: a(91, 156, 255, 60),
            ok: c(88, 196, 126),
            warn: c(240, 183, 76),
            err: c(240, 100, 100),
            zebra: a(255, 255, 255, 7),
            hover: a(255, 255, 255, 18),
            grid_line: a(255, 255, 255, 16),
            cell_mod: a(240, 183, 76, 55),
            cell_new: a(88, 196, 126, 55),
            cell_del: a(240, 100, 100, 55),
            header: c(45, 50, 61),
            number: c(125, 190, 255),
        }
    } else {
        Palette {
            bg: c(244, 245, 248),
            panel: c(255, 255, 255),
            toolbar: c(236, 239, 244),
            editor: c(255, 255, 255),
            border: c(217, 221, 229),
            text: c(31, 36, 48),
            weak: c(112, 120, 135),
            accent: c(47, 111, 224),
            accent_soft: a(47, 111, 224, 40),
            ok: c(34, 150, 84),
            warn: c(201, 130, 12),
            err: c(206, 55, 55),
            zebra: a(20, 30, 60, 8),
            hover: a(20, 30, 60, 16),
            grid_line: a(20, 30, 60, 22),
            cell_mod: a(255, 196, 60, 90),
            cell_new: a(60, 190, 110, 80),
            cell_del: a(235, 80, 80, 70),
            header: c(232, 236, 243),
            number: c(30, 90, 200),
        }
    }
}

pub fn apply(ctx: &egui::Context, dark: bool, size: f32) {
    let p = pal(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.editor;
    v.faint_bg_color = p.zebra;
    v.code_bg_color = p.toolbar;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 50 }) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 40 }) };
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.override_text_color = Some(p.text);
    v.interact_cursor = Some(egui::CursorIcon::PointingHand); // every egui button shows a hand
    let r = CornerRadius::same(6);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.inactive.weak_bg_fill = p.toolbar;
    v.widgets.inactive.bg_fill = p.toolbar;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.weak_bg_fill = p.accent_soft;
    v.widgets.active.bg_fill = p.accent_soft;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.open.weak_bg_fill = p.accent_soft;
    ctx.set_visuals(v);
    ctx.all_styles_mut(|style| {
        use egui::{FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(size + 6.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(size, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(size, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(size, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(size - 2.5, FontFamily::Proportional)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.window_margin = Margin::same(14);
        style.spacing.indent = 14.0;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.interaction.tooltip_delay = 0.4;
    });
}

/// Tries a few common system fonts for a cleaner look; falls back to egui's built-in ones.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let sans = [
        "/usr/share/fonts/TTF/Inter-Regular.ttf",
        "/usr/share/fonts/inter/Inter-Regular.otf",
        "/usr/share/fonts/TTF/FiraSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/cantarell/Cantarell-VF.otf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    let mono = [
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/TTF/FiraCode-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    ];
    if let Some((name, data)) = first_font(&sans) {
        fonts.font_data.insert(name.clone(), Arc::new(FontData::from_owned(data)));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, name);
    }
    if let Some((name, data)) = first_font(&mono) {
        fonts.font_data.insert(name.clone(), Arc::new(FontData::from_owned(data)));
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, name);
    }
    ctx.set_fonts(fonts);
}

fn first_font(paths: &[&str]) -> Option<(String, Vec<u8>)> {
    for p in paths {
        if let Ok(d) = std::fs::read(p) {
            return Some((format!("sys:{}", p), d));
        }
    }
    None
}
