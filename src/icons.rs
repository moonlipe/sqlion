//! Vector icons drawn with the painter (no emoji / icon fonts needed) and a few custom widgets.
use crate::theme::pal;
use egui::{pos2, vec2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Table,
    View,
    Database,
    Key,
    Link,
    Play,
    PlayAll,
    Plus,
    Minus,
    Check,
    Cross,
    Save,
    Undo,
    Refresh,
    Export,
    Import,
    Search,
    Script,
    Plan,
    Format,
    Folder,
    Diagram,
    Filter,
    Copy,
    Grid,
    Text,
    Record,
    Plug,
    Trash,
    Column,
    Home,
    Lock,
    Settings,
    Panel,
    Null,
    Sun,
    Moon,
    Func,
    Proc,
    Bolt,
    Seq,
    Clock,
    User,
    Type,
    Puzzle,
    Index,
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + r.width() * x, r.top() + r.height() * y)
}

fn arc(r: Rect, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * i as f32 / n as f32;
            p(r, cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

pub fn paint(pt: &Painter, r: Rect, icon: Icon, col: Color32) {
    let s = Stroke::new(1.4, col);
    let line = |pts: Vec<Pos2>| pt.add(Shape::line(pts, s));
    let fill = |pts: Vec<Pos2>| pt.add(Shape::convex_polygon(pts, col, Stroke::NONE));
    let soft = col.gamma_multiply(0.35);
    let rect_o = |a: Rect, rad: u8| pt.rect_stroke(a, CornerRadius::same(rad), s, StrokeKind::Inside);
    let sub = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(p(r, x0, y0), p(r, x1, y1));
    use std::f32::consts::PI;
    match icon {
        Icon::Table => {
            pt.rect_filled(sub(0.1, 0.15, 0.9, 0.4), CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, soft);
            rect_o(sub(0.1, 0.15, 0.9, 0.85), 2);
            line(vec![p(r, 0.1, 0.4), p(r, 0.9, 0.4)]);
            line(vec![p(r, 0.1, 0.62), p(r, 0.9, 0.62)]);
            line(vec![p(r, 0.42, 0.4), p(r, 0.42, 0.85)]);
        }
        Icon::View => {
            let mut top = arc(r, 0.5, 0.75, 0.46, 0.5, -PI * 0.84, -PI * 0.16, 14);
            let bot = arc(r, 0.5, 0.25, 0.46, 0.5, PI * 0.16, PI * 0.84, 14);
            top.extend(bot);
            top.push(top[0]);
            line(top);
            pt.circle_filled(p(r, 0.5, 0.5), r.width() * 0.13, col);
        }
        Icon::Database => {
            let mut body = vec![p(r, 0.16, 0.22)];
            body.extend(arc(r, 0.5, 0.78, 0.34, 0.12, PI, 0.0, 10));
            body.push(p(r, 0.84, 0.22));
            line(body);
            line(arc(r, 0.5, 0.22, 0.34, 0.12, 0.0, 2.0 * PI, 16));
            line(arc(r, 0.5, 0.5, 0.34, 0.12, 0.0, PI, 10));
        }
        Icon::Key => {
            line(arc(r, 0.3, 0.5, 0.2, 0.2, 0.0, 2.0 * PI, 14));
            line(vec![p(r, 0.5, 0.5), p(r, 0.92, 0.5)]);
            line(vec![p(r, 0.76, 0.5), p(r, 0.76, 0.72)]);
            line(vec![p(r, 0.9, 0.5), p(r, 0.9, 0.66)]);
        }
        Icon::Link => {
            line(vec![p(r, 0.2, 0.8), p(r, 0.78, 0.22)]);
            line(vec![p(r, 0.42, 0.2), p(r, 0.8, 0.2), p(r, 0.8, 0.58)]);
        }
        Icon::Play => {
            fill(vec![p(r, 0.28, 0.14), p(r, 0.28, 0.86), p(r, 0.86, 0.5)]);
        }
        Icon::PlayAll => {
            fill(vec![p(r, 0.08, 0.2), p(r, 0.08, 0.8), p(r, 0.5, 0.5)]);
            fill(vec![p(r, 0.5, 0.2), p(r, 0.5, 0.8), p(r, 0.92, 0.5)]);
        }
        Icon::Plus => {
            line(vec![p(r, 0.5, 0.18), p(r, 0.5, 0.82)]);
            line(vec![p(r, 0.18, 0.5), p(r, 0.82, 0.5)]);
        }
        Icon::Minus => {
            line(vec![p(r, 0.18, 0.5), p(r, 0.82, 0.5)]);
        }
        Icon::Check => {
            line(vec![p(r, 0.16, 0.54), p(r, 0.4, 0.78), p(r, 0.86, 0.24)]);
        }
        Icon::Cross => {
            line(vec![p(r, 0.24, 0.24), p(r, 0.76, 0.76)]);
            line(vec![p(r, 0.76, 0.24), p(r, 0.24, 0.76)]);
        }
        Icon::Save => {
            rect_o(sub(0.14, 0.14, 0.86, 0.86), 2);
            rect_o(sub(0.3, 0.14, 0.68, 0.38), 0);
            rect_o(sub(0.28, 0.56, 0.72, 0.86), 0);
        }
        Icon::Undo => {
            line(arc(r, 0.5, 0.58, 0.32, 0.3, PI * 1.05, PI * 2.1, 14));
            fill(vec![p(r, 0.1, 0.18), p(r, 0.1, 0.5), p(r, 0.4, 0.4)]);
        }
        Icon::Refresh => {
            line(arc(r, 0.5, 0.5, 0.34, 0.34, -PI * 0.2, PI * 1.35, 18));
            fill(vec![p(r, 0.88, 0.1), p(r, 0.9, 0.46), p(r, 0.58, 0.36)]);
        }
        Icon::Export => {
            line(vec![p(r, 0.14, 0.62), p(r, 0.14, 0.86), p(r, 0.86, 0.86), p(r, 0.86, 0.62)]);
            line(vec![p(r, 0.5, 0.7), p(r, 0.5, 0.14)]);
            line(vec![p(r, 0.3, 0.34), p(r, 0.5, 0.14), p(r, 0.7, 0.34)]);
        }
        Icon::Import => {
            line(vec![p(r, 0.14, 0.62), p(r, 0.14, 0.86), p(r, 0.86, 0.86), p(r, 0.86, 0.62)]);
            line(vec![p(r, 0.5, 0.14), p(r, 0.5, 0.7)]);
            line(vec![p(r, 0.3, 0.5), p(r, 0.5, 0.7), p(r, 0.7, 0.5)]);
        }
        Icon::Search => {
            line(arc(r, 0.42, 0.42, 0.28, 0.28, 0.0, 2.0 * PI, 18));
            line(vec![p(r, 0.62, 0.62), p(r, 0.9, 0.9)]);
        }
        Icon::Script => {
            line(vec![p(r, 0.2, 0.1), p(r, 0.62, 0.1), p(r, 0.82, 0.3), p(r, 0.82, 0.9), p(r, 0.2, 0.9), p(r, 0.2, 0.1)]);
            line(vec![p(r, 0.62, 0.1), p(r, 0.62, 0.3), p(r, 0.82, 0.3)]);
            line(vec![p(r, 0.32, 0.5), p(r, 0.7, 0.5)]);
            line(vec![p(r, 0.32, 0.68), p(r, 0.7, 0.68)]);
        }
        Icon::Plan => {
            pt.rect_filled(sub(0.14, 0.5, 0.32, 0.88), 1.0, col);
            pt.rect_filled(sub(0.41, 0.3, 0.59, 0.88), 1.0, col);
            pt.rect_filled(sub(0.68, 0.12, 0.86, 0.88), 1.0, col);
        }
        Icon::Format => {
            for (i, w) in [0.8, 0.55, 0.7, 0.4].iter().enumerate() {
                let y = 0.2 + i as f32 * 0.2;
                line(vec![p(r, 0.12, y), p(r, 0.12 + w, y)]);
            }
        }
        Icon::Folder => {
            line(vec![p(r, 0.1, 0.8), p(r, 0.1, 0.2), p(r, 0.4, 0.2), p(r, 0.5, 0.34), p(r, 0.9, 0.34), p(r, 0.9, 0.8), p(r, 0.1, 0.8)]);
        }
        Icon::Diagram => {
            rect_o(sub(0.08, 0.1, 0.4, 0.38), 1);
            rect_o(sub(0.6, 0.62, 0.92, 0.9), 1);
            rect_o(sub(0.6, 0.1, 0.92, 0.38), 1);
            line(vec![p(r, 0.4, 0.24), p(r, 0.6, 0.24)]);
            line(vec![p(r, 0.24, 0.38), p(r, 0.24, 0.76), p(r, 0.6, 0.76)]);
        }
        Icon::Filter => {
            line(vec![p(r, 0.1, 0.16), p(r, 0.9, 0.16), p(r, 0.58, 0.54), p(r, 0.58, 0.86), p(r, 0.42, 0.76), p(r, 0.42, 0.54), p(r, 0.1, 0.16)]);
        }
        Icon::Copy => {
            rect_o(sub(0.3, 0.3, 0.88, 0.88), 2);
            line(vec![p(r, 0.3, 0.7), p(r, 0.12, 0.7), p(r, 0.12, 0.12), p(r, 0.7, 0.12), p(r, 0.7, 0.3)]);
        }
        Icon::Grid => {
            rect_o(sub(0.1, 0.1, 0.9, 0.9), 2);
            line(vec![p(r, 0.1, 0.37), p(r, 0.9, 0.37)]);
            line(vec![p(r, 0.1, 0.63), p(r, 0.9, 0.63)]);
            line(vec![p(r, 0.37, 0.1), p(r, 0.37, 0.9)]);
            line(vec![p(r, 0.63, 0.1), p(r, 0.63, 0.9)]);
        }
        Icon::Text => {
            for i in 0..4 {
                let y = 0.2 + i as f32 * 0.2;
                line(vec![p(r, 0.12, y), p(r, if i == 3 { 0.5 } else { 0.88 }, y)]);
            }
        }
        Icon::Record => {
            for i in 0..3 {
                let y = 0.22 + i as f32 * 0.28;
                line(vec![p(r, 0.1, y), p(r, 0.34, y)]);
                line(vec![p(r, 0.46, y), p(r, 0.9, y)]);
            }
        }
        Icon::Plug => {
            rect_o(sub(0.28, 0.38, 0.72, 0.7), 3);
            line(vec![p(r, 0.4, 0.38), p(r, 0.4, 0.14)]);
            line(vec![p(r, 0.6, 0.38), p(r, 0.6, 0.14)]);
            line(vec![p(r, 0.5, 0.7), p(r, 0.5, 0.9)]);
        }
        Icon::Trash => {
            line(vec![p(r, 0.12, 0.26), p(r, 0.88, 0.26)]);
            line(vec![p(r, 0.38, 0.26), p(r, 0.38, 0.12), p(r, 0.62, 0.12), p(r, 0.62, 0.26)]);
            line(vec![p(r, 0.2, 0.26), p(r, 0.26, 0.9), p(r, 0.74, 0.9), p(r, 0.8, 0.26)]);
            line(vec![p(r, 0.42, 0.42), p(r, 0.42, 0.74)]);
            line(vec![p(r, 0.58, 0.42), p(r, 0.58, 0.74)]);
        }
        Icon::Column => {
            pt.circle_filled(p(r, 0.5, 0.5), r.width() * 0.12, col);
        }
        Icon::Home => {
            line(vec![p(r, 0.1, 0.5), p(r, 0.5, 0.12), p(r, 0.9, 0.5)]);
            line(vec![p(r, 0.22, 0.44), p(r, 0.22, 0.88), p(r, 0.78, 0.88), p(r, 0.78, 0.44)]);
        }
        Icon::Lock => {
            rect_o(sub(0.22, 0.44, 0.78, 0.88), 2);
            line(arc(r, 0.5, 0.44, 0.2, 0.26, PI, 2.0 * PI, 10));
        }
        Icon::Func => {
            // f(x): a curly bracket pair
            line(vec![p(r, 0.36, 0.14), p(r, 0.26, 0.2), p(r, 0.26, 0.42), p(r, 0.12, 0.5), p(r, 0.26, 0.58), p(r, 0.26, 0.8), p(r, 0.36, 0.86)]);
            line(vec![p(r, 0.64, 0.14), p(r, 0.74, 0.2), p(r, 0.74, 0.42), p(r, 0.88, 0.5), p(r, 0.74, 0.58), p(r, 0.74, 0.8), p(r, 0.64, 0.86)]);
        }
        Icon::Proc => {
            rect_o(sub(0.1, 0.12, 0.9, 0.88), 3);
            fill(vec![p(r, 0.38, 0.3), p(r, 0.38, 0.7), p(r, 0.7, 0.5)]);
        }
        Icon::Bolt => {
            fill(vec![p(r, 0.58, 0.06), p(r, 0.2, 0.56), p(r, 0.46, 0.56), p(r, 0.38, 0.94), p(r, 0.8, 0.4), p(r, 0.54, 0.4)]);
        }
        Icon::Seq => {
            for i in 0..3 {
                let x = 0.18 + i as f32 * 0.28;
                pt.rect_filled(sub(x, 0.62 - i as f32 * 0.2, x + 0.18, 0.86), 1.0, col);
            }
        }
        Icon::Clock => {
            line(arc(r, 0.5, 0.5, 0.38, 0.38, 0.0, 2.0 * PI, 20));
            line(vec![p(r, 0.5, 0.26), p(r, 0.5, 0.5), p(r, 0.68, 0.6)]);
        }
        Icon::User => {
            line(arc(r, 0.5, 0.3, 0.17, 0.17, 0.0, 2.0 * PI, 14));
            line(arc(r, 0.5, 0.92, 0.34, 0.4, PI * 1.12, PI * 1.88, 12));
        }
        Icon::Type => {
            line(vec![p(r, 0.18, 0.2), p(r, 0.82, 0.2)]);
            line(vec![p(r, 0.5, 0.2), p(r, 0.5, 0.84)]);
        }
        Icon::Puzzle => {
            rect_o(sub(0.14, 0.3, 0.86, 0.86), 2);
            rect_o(sub(0.38, 0.12, 0.62, 0.3), 2);
        }
        Icon::Index => {
            line(vec![p(r, 0.14, 0.26), p(r, 0.86, 0.26)]);
            line(vec![p(r, 0.14, 0.5), p(r, 0.66, 0.5)]);
            line(vec![p(r, 0.14, 0.74), p(r, 0.46, 0.74)]);
            fill(vec![p(r, 0.7, 0.62), p(r, 0.9, 0.62), p(r, 0.8, 0.82)]);
        }
        Icon::Sun => {
            line(arc(r, 0.5, 0.5, 0.2, 0.2, 0.0, 2.0 * PI, 14));
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                line(vec![p(r, 0.5 + 0.3 * a.cos(), 0.5 + 0.3 * a.sin()), p(r, 0.5 + 0.42 * a.cos(), 0.5 + 0.42 * a.sin())]);
            }
        }
        Icon::Moon => {
            // crescent: long outer arc, then back along a shorter inner arc
            let mut pts = arc(r, 0.5, 0.5, 0.38, 0.38, -PI / 3.0, -PI * 1.78, 22);
            pts.extend(arc(r, 0.78, 0.46, 0.29, 0.29, 1.54, 4.41, 16));
            pt.add(Shape::line(pts, s));
        }
        Icon::Settings => {
            // three sliders
            for (i, k) in [0.3f32, 0.7, 0.45].iter().enumerate() {
                let y = 0.22 + i as f32 * 0.28;
                line(vec![p(r, 0.1, y), p(r, 0.9, y)]);
                pt.circle_filled(p(r, *k, y), r.width() * 0.12, col);
            }
        }
        Icon::Panel => {
            rect_o(sub(0.1, 0.14, 0.9, 0.86), 2);
            line(vec![p(r, 0.62, 0.14), p(r, 0.62, 0.86)]);
        }
        Icon::Null => {
            line(arc(r, 0.5, 0.5, 0.3, 0.3, 0.0, 2.0 * PI, 16));
            line(vec![p(r, 0.28, 0.72), p(r, 0.72, 0.28)]);
        }
    }
}

pub fn icon_size(ui: &Ui) -> f32 {
    (ui.text_style_height(&egui::TextStyle::Body) + 2.0).max(14.0)
}

/// A small colored badge with two letters, used for the database type.
pub fn badge(pt: &Painter, rect: Rect, text: &str, color: Color32) {
    pt.rect_filled(rect, CornerRadius::same(4), color);
    pt.text(rect.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(rect.height() * 0.58), contrast(color));
}

/// Text/icon colour that stays readable on `bg`: dark on light fills, white on dark ones.
pub fn contrast(bg: Color32) -> Color32 {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(bg.r()) + 0.7152 * lin(bg.g()) + 0.0722 * lin(bg.b());
    if l > 0.40 {
        Color32::from_rgb(25, 28, 34)
    } else {
        Color32::WHITE
    }
}

/// Icons a connection can use instead of a text label: (stored name, label in the picker).
pub const PICKABLE: &[(&str, Icon)] = &[
    ("database", Icon::Database),
    ("table", Icon::Table),
    ("key", Icon::Key),
    ("script", Icon::Script),
    ("diagram", Icon::Diagram),
    ("bolt", Icon::Bolt),
    ("lock", Icon::Lock),
    ("home", Icon::Home),
    ("puzzle", Icon::Puzzle),
    ("clock", Icon::Clock),
    ("user", Icon::User),
    ("folder", Icon::Folder),
];

pub fn icon_by_name(name: &str) -> Option<Icon> {
    PICKABLE.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
}

/// A connection badge: a coloured rounded square with a vector icon or a short label.
pub fn conn_badge(pt: &Painter, rect: Rect, style: &crate::db::BadgeStyle) {
    let (r, g, b) = style.color;
    let fill = Color32::from_rgb(r, g, b);
    match style.icon.as_deref().and_then(icon_by_name) {
        Some(icon) => {
            pt.rect_filled(rect, CornerRadius::same(4), fill);
            let side = rect.height() * 0.78;
            paint(pt, Rect::from_center_size(rect.center(), vec2(side, side)), icon, contrast(fill));
        }
        None => badge(pt, rect, &style.label, fill),
    }
}

/// Flat button with an icon and optional text.
pub fn button(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    styled_button(ui, icon, text, false)
}

/// Filled accent button for the primary action.
pub fn primary(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    styled_button(ui, icon, text, true)
}

fn styled_button(ui: &mut Ui, icon: Icon, text: &str, primary: bool) -> Response {
    let pl = pal(ui.visuals().dark_mode);
    let enabled = ui.is_enabled();
    let isz = icon_size(ui);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = if text.is_empty() { None } else { Some(ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER)) };
    let pad = 8.0;
    let w = pad * 2.0 + isz + galley.as_ref().map(|g| g.size().x + 6.0).unwrap_or(0.0);
    let h = 28.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
    let hovered = resp.hovered() && enabled;
    let (bg, fg) = if primary {
        let bg = if hovered { pl.accent.gamma_multiply(1.15) } else { pl.accent };
        (bg, Color32::WHITE)
    } else {
        (if hovered { pl.hover } else { Color32::TRANSPARENT }, if enabled { pl.text } else { pl.weak.gamma_multiply(0.7) })
    };
    let pt = ui.painter();
    pt.rect_filled(rect, CornerRadius::same(6), if enabled || !primary { bg } else { pl.weak.gamma_multiply(0.4) });
    if resp.is_pointer_button_down_on() && enabled {
        pt.rect_filled(rect, CornerRadius::same(6), pl.accent_soft);
    }
    let irect = Rect::from_min_size(pos2(rect.left() + pad, rect.center().y - isz / 2.0), vec2(isz, isz));
    paint(pt, irect, icon, if primary || !enabled { fg } else { icon_color(icon, &pl, fg) });
    if let Some(g) = galley {
        pt.galley(pos2(irect.right() + 6.0, rect.center().y - g.size().y / 2.0), g, fg);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Gives actions a recognisable tint (green play, red trash...).
pub fn icon_color(icon: Icon, pl: &crate::theme::Palette, fallback: Color32) -> Color32 {
    match icon {
        Icon::Play | Icon::PlayAll | Icon::Check | Icon::Plus => pl.ok,
        Icon::Trash | Icon::Cross | Icon::Minus => pl.err,
        Icon::Save | Icon::Undo => pl.warn,
        Icon::Key => pl.warn,
        Icon::Table | Icon::View | Icon::Database | Icon::Search | Icon::Refresh | Icon::Export | Icon::Import | Icon::Diagram | Icon::Plug => pl.accent,
        _ => fallback,
    }
}

/// Segmented-control style toggle button.
pub fn segment(ui: &mut Ui, icon: Icon, text: &str, active: bool) -> Response {
    let pl = pal(ui.visuals().dark_mode);
    let isz = icon_size(ui) - 2.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let g = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let w = 10.0 * 2.0 + isz + g.size().x + 6.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    let pt = ui.painter();
    let bg = if active { pl.accent_soft } else if resp.hovered() { pl.hover } else { Color32::TRANSPARENT };
    pt.rect_filled(rect, CornerRadius::same(6), bg);
    if active {
        pt.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, pl.accent), StrokeKind::Inside);
    }
    let col = if active { pl.accent } else { pl.text };
    let irect = Rect::from_min_size(pos2(rect.left() + 10.0, rect.center().y - isz / 2.0), vec2(isz, isz));
    paint(pt, irect, icon, col);
    pt.galley(pos2(irect.right() + 6.0, rect.center().y - g.size().y / 2.0), g, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row for the navigator tree: icon + text + optional dim suffix.
pub enum Lead<'a> {
    Icon(Icon, Color32),
    Badge(&'a str, Color32),
    Conn(&'a crate::db::BadgeStyle),
}

pub fn tree_row(ui: &mut Ui, lead: Lead, dot: Option<Color32>, text: RichText, suffix: &str, selected: bool, tint: Option<Color32>) -> Response {
    let pl = pal(ui.visuals().dark_mode);
    let isz = icon_size(ui) - 1.0;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let g = ui.painter().layout_no_wrap(text.text().to_string(), font.clone(), Color32::PLACEHOLDER);
    let gs = if suffix.is_empty() { None } else { Some(ui.painter().layout_no_wrap(suffix.to_string(), egui::TextStyle::Small.resolve(ui.style()), Color32::PLACEHOLDER)) };
    let w = (isz + 18.0 + g.size().x + gs.as_ref().map(|s| s.size().x + 8.0).unwrap_or(0.0) + 12.0).max(60.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click_and_drag());
    let pt = ui.painter();
    if let Some(t) = tint {
        pt.rect_filled(rect, CornerRadius::same(5), t);
    }
    if selected {
        pt.rect_filled(rect, CornerRadius::same(5), pl.accent_soft);
    } else if resp.hovered() {
        pt.rect_filled(rect, CornerRadius::same(5), pl.hover);
    }
    let mut x0 = rect.left() + 4.0;
    if let Some(d) = dot {
        pt.circle_filled(pos2(x0 + 3.0, rect.center().y), 3.0, d);
        x0 += 10.0;
    }
    let irect = match lead {
        Lead::Icon(icon, col) => {
            let r = Rect::from_min_size(pos2(x0, rect.center().y - isz / 2.0), vec2(isz, isz));
            paint(pt, r, icon, col);
            r
        }
        Lead::Badge(t, col) => {
            let r = Rect::from_min_size(pos2(x0, rect.center().y - isz / 2.0), vec2(isz + 6.0, isz));
            badge(pt, r, t, col);
            r
        }
        Lead::Conn(style) => {
            let r = Rect::from_min_size(pos2(x0, rect.center().y - isz / 2.0), vec2(isz + 6.0, isz));
            conn_badge(pt, r, style);
            r
        }
    };
    let tc = if tint.is_some() { Color32::from_rgb(35, 38, 45) } else { pl.text };
    pt.galley(pos2(irect.right() + 6.0, rect.center().y - g.size().y / 2.0), g.clone(), tc);
    if let Some(s) = gs {
        pt.galley(pos2(irect.right() + 6.0 + g.size().x + 8.0, rect.center().y - s.size().y / 2.0), s, pl.weak);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// An editor tab; returns (select response, close clicked).
pub fn tab_item(ui: &mut Ui, icon: Icon, title: &str, active: bool, dirty: bool) -> (Response, bool) {
    let pl = pal(ui.visuals().dark_mode);
    let isz = icon_size(ui) - 1.0;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let g = ui.painter().layout_no_wrap(title.to_string(), font, Color32::PLACEHOLDER);
    let w = 12.0 + isz + 6.0 + g.size().x + 10.0 + 16.0 + 8.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    let close_rect = Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(16.0, 16.0));
    let close_resp = ui.interact(close_rect, resp.id.with("close"), Sense::click());
    let pt = ui.painter();
    let bg = if active { pl.editor } else if resp.hovered() { pl.hover } else { Color32::TRANSPARENT };
    pt.rect_filled(rect, CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, bg);
    if active {
        pt.rect_filled(Rect::from_min_size(rect.left_top(), vec2(rect.width(), 2.5)), CornerRadius::ZERO, pl.accent);
    }
    let col = if active { pl.text } else { pl.weak };
    let irect = Rect::from_min_size(pos2(rect.left() + 12.0, rect.center().y - isz / 2.0), vec2(isz, isz));
    paint(pt, irect, icon, if active { icon_color(icon, &pl, col) } else { col });
    pt.galley(pos2(irect.right() + 6.0, rect.center().y - g.size().y / 2.0), g, col);
    if dirty && !close_resp.hovered() {
        pt.circle_filled(close_rect.center(), 3.5, pl.warn);
    } else if resp.hovered() || active || close_resp.hovered() {
        if close_resp.hovered() {
            pt.rect_filled(close_rect, CornerRadius::same(4), pl.hover);
        }
        paint(pt, close_rect.shrink(3.5), Icon::Cross, if close_resp.hovered() { pl.text } else { pl.weak });
    }
    let close_clicked = close_resp.clicked();
    let _ = close_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    (resp.on_hover_cursor(egui::CursorIcon::PointingHand), close_clicked)
}

/// Thin section title for panels.
pub fn section(ui: &mut Ui, text: &str) {
    let pl = pal(ui.visuals().dark_mode);
    ui.label(RichText::new(text.to_uppercase()).small().strong().color(pl.weak));
}

/// A chip (rounded label) such as "somente leitura".
pub fn chip(ui: &mut Ui, icon: Option<Icon>, text: &str, color: Color32) {
    let isz = icon_size(ui) - 3.0;
    let font = egui::TextStyle::Small.resolve(ui.style());
    let g = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let w = 16.0 + g.size().x + if icon.is_some() { isz + 4.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let pt = ui.painter();
    pt.rect_filled(rect, CornerRadius::same(11), color.gamma_multiply(0.16));
    let mut x = rect.left() + 8.0;
    if let Some(i) = icon {
        paint(pt, Rect::from_min_size(pos2(x, rect.center().y - isz / 2.0), vec2(isz, isz)), i, color);
        x += isz + 4.0;
    }
    pt.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{CursorIcon, Event, RawInput};

    fn run(ctx: &egui::Context, pos: Option<Pos2>, t: f64, rect_out: &mut Option<Rect>, what: u8) -> CursorIcon {
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 300.0)));
        input.time = Some(t);
        if let Some(p) = pos {
            input.events.push(Event::PointerMoved(p));
        }
        let mut out = ctx.run_ui(input, |ui| {
            let r = match what {
                0 => button(ui, Icon::Settings, ""),
                1 => button(ui, Icon::Moon, "Escuro"),
                2 => ui.add(egui::Button::new("padrão")),
                _ => segment(ui, Icon::Grid, "Grade", false),
            };
            *rect_out = Some(r.rect);
        });
        out.textures_delta.clear();
        out.platform_output.cursor_icon
    }

    #[test]
    fn hand_cursor_over_buttons() {
        for what in 0..4u8 {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx, true, 14.0);
            let mut rect = None;
            let mut t = 0.0;
            for _ in 0..2 {
                t += 0.05;
                run(&ctx, None, t, &mut rect, what);
            }
            let center = rect.unwrap().center();
            assert_eq!(run(&ctx, Some(Pos2::new(5.0, 250.0)), t + 0.05, &mut rect, what), CursorIcon::Default, "away from the button");
            let mut cur = CursorIcon::Default;
            for i in 0..3 {
                cur = run(&ctx, Some(center), t + 0.1 + i as f64 * 0.05, &mut rect, what);
            }
            assert_eq!(cur, CursorIcon::PointingHand, "button kind {}", what);
        }
    }
}
