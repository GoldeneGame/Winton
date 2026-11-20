//! Couleurs, polices et petits widgets dessinés à la main.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Rect, Response, RichText,
    Rounding, Sense, Shape, Stroke, TextStyle, TextureHandle, Ui,
};
use std::path::PathBuf;

pub const BG: Color32 = Color32::from_rgb(0x0F, 0x11, 0x15);
pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x19, 0x20);
pub const CARD: Color32 = Color32::from_rgb(0x1F, 0x23, 0x2C);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x27, 0x2C, 0x37);
pub const LINE: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x3A);
pub const TEXT: Color32 = Color32::from_rgb(0xE8, 0xEA, 0xF0);
pub const SUB: Color32 = Color32::from_rgb(0x8C, 0x94, 0xA6);
pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8C, 0xFF);
pub const PLAY: Color32 = Color32::from_rgb(0x2F, 0xBF, 0x71);
pub const DANGER: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);
pub const FALLBACK: Color32 = Color32::from_rgb(0x25, 0x2B, 0x39);
pub const INITIAL: Color32 = Color32::from_rgb(0x4A, 0x55, 0x70);

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold())
}

pub fn setup(ctx: &egui::Context) {
    // Polices Windows (Segoe UI) si disponibles, sinon celles d'egui.
    let mut fonts = FontDefinitions::default();
    let dir = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    let proportional = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut regular = proportional.clone();
    let mut bold_list = proportional;
    if let Ok(bytes) = std::fs::read(dir.join("segoeui.ttf")) {
        fonts.font_data.insert("segoe".into(), FontData::from_owned(bytes));
        regular.insert(0, "segoe".into());
    }
    if let Ok(bytes) = std::fs::read(dir.join("segoeuib.ttf")) {
        fonts.font_data.insert("segoe-bold".into(), FontData::from_owned(bytes));
        bold_list.insert(0, "segoe-bold".into());
    }
    fonts.families.insert(FontFamily::Proportional, regular);
    fonts.families.insert(bold(), bold_list);
    ctx.set_fonts(fonts);

    ctx.options_mut(|o| o.theme_preference = egui::ThemePreference::Dark);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = CARD;
    v.faint_bg_color = CARD;
    v.window_rounding = Rounding::same(12.0);
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    v.menu_rounding = Rounding::same(8.0);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xDD, 0xE6, 0xFF));
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(8.0);
        w.fg_stroke = Stroke::new(1.0_f32, TEXT);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.inactive.weak_bg_fill = CARD;
    v.widgets.inactive.bg_fill = CARD;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.hovered.weak_bg_fill = CARD_HOVER;
    v.widgets.hovered.bg_fill = CARD_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x3A, 0x40, 0x4D));
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x2E, 0x34, 0x40);
    v.widgets.active.bg_fill = Color32::from_rgb(0x2E, 0x34, 0x40);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = vec2(14.0, 8.0);
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(11.5));
    style.text_styles.insert(TextStyle::Heading, bold_font(22.0));
    ctx.set_style(style);
}

// ------------------------------------------------------------------ boutons

pub fn filled_button(text: &str, fill: Color32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).font(bold_font(14.0)))
        .fill(fill)
        .stroke(Stroke::NONE)
        .rounding(8.0)
}

pub fn danger_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(DANGER))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x5A, 0x2A, 0x2E)))
        .rounding(8.0)
}

/// Pastille de tri (A–Z, Récents…).
pub fn pill(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.5), TEXT);
    let size = galley.size() + vec2(24.0, 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let color = if selected || resp.hovered() { TEXT } else { SUB };
    if selected {
        ui.painter().rect_filled(rect, 15.0, CARD);
    }
    ui.painter().galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Petit carré bleu avec triangle « lecture » (logo).
pub fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().rect_filled(rect, size * 0.27, ACCENT);
    play_triangle(ui.painter(), rect.center() + vec2(size * 0.05, 0.0), size * 0.22, Color32::WHITE);
}

pub fn play_triangle(p: &egui::Painter, center: egui::Pos2, r: f32, color: Color32) {
    let pts = vec![
        center + vec2(-r * 0.6, -r),
        center + vec2(r, 0.0),
        center + vec2(-r * 0.6, r),
    ];
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

// ------------------------------------------------------------------ visuels de jeu

/// Jaquette (recadrée pour remplir) ou, à défaut, icône du jeu ou initiale sur fond uni.
pub fn paint_art(
    ui: &Ui,
    rect: Rect,
    cover: Option<&TextureHandle>,
    icon: Option<&TextureHandle>,
    initial: &str,
    rounding: f32,
    icon_size: f32,
    offset_y: f32,
) {
    let p = ui.painter();
    p.rect_filled(rect, rounding, FALLBACK);
    if let Some(t) = cover {
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(t))
            .uv(cover_uv(t.size_vec2(), rect.size()))
            .rounding(rounding)
            .paint_at(ui, rect);
        return;
    }
    let c = rect.center() + vec2(0.0, offset_y);
    if let Some(t) = icon {
        let r = Rect::from_center_size(c, vec2(icon_size, icon_size));
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(t)).paint_at(ui, r);
    } else {
        p.text(c, Align2::CENTER_CENTER, initial, bold_font(icon_size * 0.85), INITIAL);
    }
}

/// Coordonnées de texture pour un remplissage « UniformToFill » (centré, recadré).
fn cover_uv(tex: egui::Vec2, target: egui::Vec2) -> Rect {
    let ta = tex.x / tex.y.max(1.0);
    let ra = target.x / target.y.max(1.0);
    if ta > ra {
        let w = ra / ta;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = ta / ra;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    }
}

/// Dégradé vertical (transparent en haut -> `bottom` en bas).
pub fn vertical_gradient(p: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// Texte sur une ligne, tronqué avec « … ».
pub fn truncated(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.fonts(|f| f.layout_job(job))
}
