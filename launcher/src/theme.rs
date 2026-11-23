//! Thème médiéval : couleurs, polices, textures générées (bois, pierre, parchemin)
//! et petits ornements dessinés à la main (cadres dorés, écus, sceaux de cire).

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response,
    RichText, Rounding, Sense, Shape, Stroke, TextStyle, TextureHandle, Ui,
};
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::OnceLock;

// ------------------------------------------------------------------ couleurs

pub const BG: Color32 = Color32::from_rgb(0x16, 0x10, 0x0B);
pub const PANEL: Color32 = Color32::from_rgb(0x24, 0x18, 0x0F);
pub const CARD: Color32 = Color32::from_rgb(0x2C, 0x1F, 0x14);
pub const LINE: Color32 = Color32::from_rgb(0x4A, 0x36, 0x21);
pub const TEXT: Color32 = Color32::from_rgb(0xEC, 0xDD, 0xBA);
pub const SUB: Color32 = Color32::from_rgb(0xA8, 0x92, 0x6A);
pub const GOLD: Color32 = Color32::from_rgb(0xC9, 0xA0, 0x4A);
pub const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xF2, 0xD2, 0x7C);
pub const GOLD_DIM: Color32 = Color32::from_rgb(0x7A, 0x5E, 0x2E);
pub const ACCENT: Color32 = GOLD;
pub const PLAY: Color32 = Color32::from_rgb(0x8E, 0x1C, 0x1C); // rouge cire
pub const PLAY_HOVER: Color32 = Color32::from_rgb(0xAE, 0x26, 0x24);
pub const RUNNING: Color32 = Color32::from_rgb(0x3F, 0x6B, 0x2E); // vert héraldique
pub const DANGER: Color32 = Color32::from_rgb(0xC0, 0x4E, 0x40);
pub const INK: Color32 = Color32::from_rgb(0x38, 0x22, 0x10);
pub const INK_SUB: Color32 = Color32::from_rgb(0x6E, 0x52, 0x33);
pub const FALLBACK: Color32 = Color32::from_rgb(0x2B, 0x1B, 0x12);
pub const INITIAL: Color32 = GOLD;

// ------------------------------------------------------------------ polices

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold())
}
/// Capitales gravées (Cinzel) : titres, boutons, noms de jeux.
pub fn heading_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("heading".into()))
}
/// Capitales ornées (Cinzel Decorative) : le nom du launcher.
pub fn title_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("title".into()))
}

fn load_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let dir = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();

    let add_system = |fonts: &mut FontDefinitions, key: &str, files: &[&str]| -> Vec<String> {
        let mut out = Vec::new();
        for f in files {
            if let Ok(bytes) = std::fs::read(dir.join(f)) {
                let name = format!("{key}-{f}");
                fonts.font_data.insert(name.clone(), FontData::from_owned(bytes));
                out.push(name);
            }
        }
        out
    };
    // Serif « livre ancien » pour le texte, Segoe UI en secours pour les symboles.
    let regular = add_system(&mut fonts, "r", &["pala.ttf", "georgia.ttf", "segoeui.ttf", "seguisym.ttf"]);
    let bold_l = add_system(&mut fonts, "b", &["palab.ttf", "georgiab.ttf", "segoeuib.ttf", "seguisym.ttf"]);

    fonts.font_data.insert(
        "cinzel".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Cinzel.ttf")),
    );
    fonts.font_data.insert(
        "cinzel-deco".into(),
        FontData::from_static(include_bytes!("../assets/fonts/CinzelDecorative-Bold.ttf")),
    );

    let chain = |first: &[&str], rest: &[String]| -> Vec<String> {
        first.iter().map(|s| s.to_string()).chain(rest.iter().cloned()).chain(defaults.iter().cloned()).collect()
    };
    fonts.families.insert(FontFamily::Proportional, chain(&[], &regular));
    fonts.families.insert(bold(), chain(&[], &bold_l));
    fonts.families.insert(FontFamily::Name("heading".into()), chain(&["cinzel"], &bold_l));
    fonts.families.insert(FontFamily::Name("title".into()), chain(&["cinzel-deco", "cinzel"], &bold_l));
    fonts
}

// ------------------------------------------------------------------ textures générées

struct Skin {
    wood: TextureHandle,
    stone: TextureHandle,
    parchment: TextureHandle,
}

static SKIN: OnceLock<Skin> = OnceLock::new();

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Bruit de valeur répétable (périodes px × py).
fn vnoise(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(fx), s(fy));
    let h = |a: i32, b: i32| hash(a.rem_euclid(px), b.rem_euclid(py), seed);
    let top = h(xi, yi) + (h(xi + 1, yi) - h(xi, yi)) * u;
    let bot = h(xi, yi + 1) + (h(xi + 1, yi + 1) - h(xi, yi + 1)) * u;
    top + (bot - top) * v
}

/// Bruit fractal sur [0,1)² répétable.
fn fbm(u: f32, v: f32, px: i32, py: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 0.5, 0.0, 1);
    for o in 0..octaves {
        sum += amp * vnoise(u * (px * f) as f32, v * (py * f) as f32, px * f, py * f, seed + o);
        norm += amp;
        amp *= 0.5;
        f *= 2;
    }
    sum / norm
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn gen(w: usize, h: usize, f: impl Fn(f32, f32) -> [f32; 3]) -> egui::ColorImage {
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let c = f(x as f32 / w as f32, y as f32 / h as f32);
            px.push(Color32::from_rgb(c[0] as u8, c[1] as u8, c[2] as u8));
        }
    }
    egui::ColorImage { size: [w, h], pixels: px }
}

fn make_skin(ctx: &egui::Context) -> Skin {
    // Planches de bois sombre, veinage horizontal, 4 planches par motif.
    let wood = gen(512, 256, |u, v| {
        let plank = (v * 4.0).floor();
        let n = fbm(u, v, 3, 12, 4, 11 + plank as u32);
        let grain = 0.5 + 0.5 * ((v * 48.0 + n * 5.0 + plank * 1.7) * TAU).sin();
        let fine = fbm(u, v, 64, 96, 2, 5);
        let mut c = mix([26.0, 17.0, 10.0], [58.0, 39.0, 22.0], grain * 0.55 + n * 0.35 + fine * 0.2);
        let edge = (v * 4.0).fract();
        let seam_x = (u + hash(plank as i32, 0, 77)).fract();
        if !(0.025..=0.975).contains(&edge) || seam_x < 0.004 {
            c = mix(c, [8.0, 5.0, 3.0], if seam_x < 0.004 { 0.45 } else { 0.8 });
        }
        c
    });
    // Pierre / cuir sombre pour le fond.
    let stone = gen(256, 256, |u, v| {
        let big = fbm(u, v, 4, 4, 5, 21);
        let grain = fbm(u, v, 64, 64, 2, 31);
        mix([16.0, 11.0, 7.0], [38.0, 27.0, 17.0], big * 0.8 + grain * 0.25 - 0.05)
    });
    // Parchemin taché.
    let parchment = gen(256, 256, |u, v| {
        let big = fbm(u, v, 3, 3, 5, 41);
        let blot = fbm(u, v, 6, 6, 3, 51);
        let fiber = fbm(u, v, 8, 96, 2, 61);
        let c = mix([196.0, 167.0, 118.0], [234.0, 216.0, 172.0], big * 0.9 + fiber * 0.25 - 0.1);
        mix(c, [150.0, 112.0, 66.0], ((blot - 0.62) * 3.0).max(0.0))
    });
    let opt = egui::TextureOptions::LINEAR_REPEAT;
    Skin {
        wood: ctx.load_texture("skin-wood", wood, opt),
        stone: ctx.load_texture("skin-stone", stone, opt),
        parchment: ctx.load_texture("skin-parchment", parchment, opt),
    }
}

#[derive(Clone, Copy)]
pub enum Surface {
    Wood,
    Stone,
    Parchment,
}

/// Remplit `rect` avec une texture répétée (alignée sur l'écran, pas d'étirement).
pub fn surface_shape(surface: Surface, rect: Rect, tint: Color32) -> Shape {
    let Some(skin) = SKIN.get() else { return Shape::rect_filled(rect, 0.0, PANEL) };
    let tex = match surface {
        Surface::Wood => &skin.wood,
        Surface::Stone => &skin.stone,
        Surface::Parchment => &skin.parchment,
    };
    let s = tex.size_vec2();
    let uv = Rect::from_min_max(
        pos2(rect.left() / s.x, rect.top() / s.y),
        pos2(rect.right() / s.x, rect.bottom() / s.y),
    );
    Shape::image(tex.id(), rect, uv, tint)
}

// ------------------------------------------------------------------ style global

pub fn setup(ctx: &egui::Context) {
    ctx.set_fonts(load_fonts());
    let _ = SKIN.set(make_skin(ctx));
    ctx.options_mut(|o| o.theme_preference = egui::ThemePreference::Dark);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = Color32::from_rgb(0x12, 0x0C, 0x08);
    v.faint_bg_color = CARD;
    v.code_bg_color = CARD;
    v.window_rounding = Rounding::same(3.0);
    v.window_stroke = Stroke::new(1.0_f32, GOLD_DIM);
    v.window_shadow = egui::epaint::Shadow { offset: vec2(0.0, 10.0), blur: 30.0, spread: 0.0, color: Color32::from_black_alpha(160) };
    v.popup_shadow = egui::epaint::Shadow { offset: vec2(0.0, 6.0), blur: 14.0, spread: 0.0, color: Color32::from_black_alpha(150) };
    v.menu_rounding = Rounding::same(3.0);
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xC9, 0xA0, 0x4A, 110);
    v.selection.stroke = Stroke::new(1.0_f32, GOLD_BRIGHT);
    v.hyperlink_color = GOLD_BRIGHT;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(3.0);
        w.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x32, 0x23, 0x16);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x32, 0x23, 0x16);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, GOLD_DIM);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x42, 0x2E, 0x1C);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x42, 0x2E, 0x1C);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GOLD);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, GOLD_BRIGHT);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x50, 0x38, 0x21);
    v.widgets.active.bg_fill = Color32::from_rgb(0x50, 0x38, 0x21);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, GOLD_BRIGHT);
    v.widgets.open = v.widgets.hovered;
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = vec2(14.0, 8.0);
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.text_styles.insert(TextStyle::Body, FontId::proportional(15.0));
    style.text_styles.insert(TextStyle::Button, heading_font(13.5));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(TextStyle::Heading, heading_font(22.0));
    ctx.set_style(style);
}

// ------------------------------------------------------------------ ornements

fn diamond(center: Pos2, r: f32, fill: Color32) -> Shape {
    Shape::convex_polygon(
        vec![center + vec2(0.0, -r), center + vec2(r, 0.0), center + vec2(0.0, r), center + vec2(-r, 0.0)],
        fill,
        Stroke::new(1.0_f32, Color32::from_black_alpha(140)),
    )
}

/// Cadre doré double avec losanges aux coins.
pub fn ornate_frame_shapes(rect: Rect, gold: Color32, corner: f32) -> Vec<Shape> {
    let faint = Color32::from_rgba_unmultiplied(gold.r(), gold.g(), gold.b(), 120);
    let mut s = vec![
        Shape::rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::from_black_alpha(200))),
        Shape::rect_stroke(rect.shrink(1.5), 2.0, Stroke::new(2.0_f32, gold)),
        Shape::rect_stroke(rect.shrink(5.0), 1.0, Stroke::new(1.0_f32, faint)),
    ];
    if corner > 0.0 {
        let r = rect.shrink(1.5);
        for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
            s.push(diamond(c, corner, gold));
            s.push(Shape::circle_filled(c, corner * 0.3, Color32::from_rgb(0x2A, 0x14, 0x0A)));
        }
    }
    s
}

pub fn ornate_frame(p: &egui::Painter, rect: Rect, gold: Color32, corner: f32) {
    p.extend(ornate_frame_shapes(rect, gold, corner));
}

/// Ligne horizontale qui s'estompe vers les bords.
fn fading_line(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32, thick: f32, fade_from_a: bool) {
    let clear = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0);
    let (ca, cb) = if fade_from_a { (clear, color) } else { (color, clear) };
    let mut mesh = egui::Mesh::default();
    let h = vec2(0.0, thick / 2.0);
    mesh.colored_vertex(a - h, ca);
    mesh.colored_vertex(b - h, cb);
    mesh.colored_vertex(a + h, ca);
    mesh.colored_vertex(b + h, cb);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// Séparateur : deux filets qui s'estompent et un losange au centre.
pub fn divider(ui: &mut Ui, color: Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 14.0), Sense::hover());
    paint_divider(ui.painter(), rect.left(), rect.right(), rect.center().y, color, 5.0);
}

pub fn paint_divider(p: &egui::Painter, left: f32, right: f32, y: f32, color: Color32, size: f32) {
    let c = pos2((left + right) / 2.0, y);
    let gap = size * 2.4;
    fading_line(p, pos2(left, y), pos2(c.x - gap, y), color, 1.2, true);
    fading_line(p, pos2(c.x + gap, y), pos2(right, y), color, 1.2, false);
    p.add(diamond(c, size, color));
    p.circle_filled(c + vec2(-size * 2.0, 0.0), size * 0.32, color);
    p.circle_filled(c + vec2(size * 2.0, 0.0), size * 0.32, color);
}

/// Filet doré sous un panneau (bas du bandeau, haut du pied de page…).
pub fn gold_rule(p: &egui::Painter, left: f32, right: f32, y: f32) {
    p.line_segment([pos2(left, y), pos2(right, y)], Stroke::new(3.0_f32, Color32::from_rgb(0x0C, 0x07, 0x04)));
    p.line_segment([pos2(left, y - 1.0), pos2(right, y - 1.0)], Stroke::new(1.0_f32, GOLD_DIM));
    p.line_segment([pos2(left, y + 1.5), pos2(right, y + 1.5)], Stroke::new(1.0_f32, Color32::from_rgb(0x5A, 0x44, 0x22)));
}

/// Écu héraldique (pointe en bas) inscrit dans `rect`.
pub fn shield_points(rect: Rect) -> Vec<Pos2> {
    let (l, r, t, b, cx) = (rect.left(), rect.right(), rect.top(), rect.bottom(), rect.center().x);
    let shoulder = t + rect.height() * 0.42;
    let mut pts = vec![pos2(l, t), pos2(r, t), pos2(r, shoulder)];
    let n = 12;
    // côté droit -> pointe (Bézier quadratique), puis côté gauche en miroir
    for i in 1..=n {
        let k = i as f32 / n as f32;
        let (p0, c, p1) = (pos2(r, shoulder), pos2(r, t + rect.height() * 0.82), pos2(cx, b));
        pts.push(p0 + (c - p0) * (2.0 * k * (1.0 - k)) + (p1 - p0) * (k * k));
    }
    for i in (0..n).rev() {
        let k = i as f32 / n as f32;
        let (p0, c, p1) = (pos2(l, shoulder), pos2(l, t + rect.height() * 0.82), pos2(cx, b));
        pts.push(p0 + (c - p0) * (2.0 * k * (1.0 - k)) + (p1 - p0) * (k * k));
    }
    pts
}

pub fn shield(p: &egui::Painter, rect: Rect, fill: Color32, rim: Color32) {
    p.add(Shape::convex_polygon(shield_points(rect.translate(vec2(0.0, 2.0))), Color32::from_black_alpha(120), Stroke::NONE));
    p.add(Shape::convex_polygon(shield_points(rect), fill, Stroke::new(2.0_f32, rim)));
    let inner = rect.shrink(rect.width() * 0.12);
    p.add(Shape::closed_line(shield_points(inner), Stroke::new(1.0_f32, rim.gamma_multiply(0.55))));
}

/// Sceau de cire rouge avec triangle « lecture ».
pub fn wax_seal(p: &egui::Painter, center: Pos2, r: f32, hovered: bool) {
    let wax = if hovered { PLAY_HOVER } else { PLAY };
    p.circle_filled(center + vec2(0.0, 3.0), r * 1.05, Color32::from_black_alpha(130));
    for i in 0..12 {
        let a = i as f32 / 12.0 * TAU;
        p.circle_filled(center + vec2(a.cos(), a.sin()) * r * 0.86, r * 0.2, wax);
    }
    p.circle_filled(center, r * 0.92, wax);
    p.circle_stroke(center, r * 0.68, Stroke::new(2.0_f32, Color32::from_rgb(0x5E, 0x0F, 0x10)));
    p.circle_stroke(center, r * 0.62, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xFF, 0xB0, 0x90, 50)));
    play_triangle(p, center + vec2(r * 0.07, 0.0), r * 0.34, GOLD_BRIGHT);
}

pub fn play_triangle(p: &egui::Painter, center: Pos2, r: f32, color: Color32) {
    let pts = vec![center + vec2(-r * 0.6, -r), center + vec2(r, 0.0), center + vec2(-r * 0.6, r)];
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Logo : écu de gueules (rouge) bordé d'or, frappé d'un W d'or (même dessin que l'icône de l'exe).
pub fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size * 0.86, size), Sense::hover());
    let p = ui.painter();
    shield(p, rect, PLAY, GOLD);
    let galley = p.layout_no_wrap("W".into(), title_font(size * 0.62), GOLD_BRIGHT);
    let center = rect.center() + vec2(0.0, -size * 0.06);
    p.galley(center - galley.size() / 2.0, galley, GOLD_BRIGHT);
}

// ------------------------------------------------------------------ boutons

fn is_light(c: Color32) -> bool {
    (c.r() as u32 * 3 + c.g() as u32 * 6 + c.b() as u32) / 10 > 140
}

/// Bouton plein (or ou rouge) bordé d'or.
pub fn filled_button(text: &str, fill: Color32) -> egui::Button<'static> {
    let color = if is_light(fill) { INK } else { TEXT };
    let rim = if is_light(fill) { Color32::from_rgb(0x5E, 0x44, 0x1C) } else { GOLD };
    egui::Button::new(RichText::new(text).color(color).font(heading_font(14.0)).strong())
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, rim))
        .rounding(3.0)
}

pub fn danger_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(DANGER).font(heading_font(13.5)))
        .fill(Color32::from_rgb(0x2A, 0x14, 0x0F))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x6A, 0x2A, 0x22)))
        .rounding(3.0)
}

/// Grand bouton « JOUER » : rouge cire, double cadre doré, losanges.
pub fn grand_button(ui: &mut Ui, size: egui::Vec2, text: &str, fill: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let p = ui.painter();
    let hot = resp.hovered();
    let top = if hot { fill.gamma_multiply(1.25) } else { fill };
    let bottom = fill.gamma_multiply(0.55);
    p.rect_filled(rect.translate(vec2(0.0, 3.0)), 3.0, Color32::from_black_alpha(140));
    vertical_gradient(p, rect, top, bottom);
    ornate_frame(p, rect, if hot { GOLD_BRIGHT } else { GOLD }, 4.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), heading_font(20.0), TEXT);
    let pos = rect.center() - galley.size() / 2.0;
    p.galley(pos + vec2(0.0, 1.5), galley.clone(), Color32::from_black_alpha(180));
    p.galley_with_override_text_color(pos, galley.clone(), if hot { GOLD_BRIGHT } else { TEXT });
    let gap = galley.size().x / 2.0 + 16.0;
    for dir in [-1.0, 1.0] {
        let c = rect.center() + vec2(dir * gap, 0.0);
        p.add(diamond(c, 4.0, GOLD));
        p.line_segment([c + vec2(dir * 7.0, 0.0), c + vec2(dir * 26.0, 0.0)], Stroke::new(1.0_f32, GOLD_DIM));
    }
    resp
}

/// Pastille de tri (A–Z, Récents…) : une bannière dorée quand elle est choisie.
pub fn pill(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), heading_font(12.5), TEXT);
    let size = galley.size() + vec2(30.0, 14.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let color = if selected { GOLD_BRIGHT } else if resp.hovered() { TEXT } else { SUB };
    if selected || resp.hovered() {
        // bannière à queues d'aronde
        let (l, r, t, b, cy) = (rect.left(), rect.right(), rect.top(), rect.bottom(), rect.center().y);
        let notch = 6.0;
        let pts = vec![
            pos2(l, t), pos2(r, t), pos2(r - notch, cy), pos2(r, b), pos2(l, b), pos2(l + notch, cy),
        ];
        let fill = if selected { Color32::from_rgb(0x5A, 0x16, 0x14) } else { Color32::from_rgb(0x30, 0x21, 0x15) };
        // remplissage en deux trapèzes convexes
        p.add(Shape::convex_polygon(vec![pts[0], pts[1], pts[2], pts[5]], fill, Stroke::NONE));
        p.add(Shape::convex_polygon(vec![pts[5], pts[2], pts[3], pts[4]], fill, Stroke::NONE));
        if selected {
            p.add(Shape::closed_line(pts, Stroke::new(1.0_f32, GOLD)));
        }
    }
    p.galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ------------------------------------------------------------------ visuels de jeu

/// Jaquette (recadrée pour remplir) ou, à défaut, icône du jeu / écu à l'initiale.
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
    if let Some(t) = cover {
        p.rect_filled(rect, rounding, FALLBACK);
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(t))
            .uv(cover_uv(t.size_vec2(), rect.size()))
            .rounding(rounding)
            .paint_at(ui, rect);
        return;
    }
    // Fond : tenture pourpre sombre sur pierre.
    p.add(surface_shape(Surface::Stone, rect, Color32::WHITE));
    vertical_gradient(p, rect, Color32::from_rgba_unmultiplied(0x5A, 0x16, 0x14, 150), Color32::from_black_alpha(90));
    let c = rect.center() + vec2(0.0, offset_y);
    if let Some(t) = icon {
        let glow = icon_size * 0.7;
        p.circle_filled(c, glow, Color32::from_rgba_unmultiplied(0xC9, 0xA0, 0x4A, 6));
        let r = Rect::from_center_size(c, vec2(icon_size, icon_size));
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(t)).paint_at(ui, r);
    } else {
        let sr = Rect::from_center_size(c, vec2(icon_size * 0.9, icon_size * 1.05));
        shield(p, sr, Color32::from_rgb(0x6E, 0x18, 0x16), GOLD);
        p.text(
            sr.center() - vec2(0.0, icon_size * 0.06),
            Align2::CENTER_CENTER,
            initial,
            title_font(icon_size * 0.5),
            INITIAL,
        );
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

/// Dégradé vertical (`top` en haut -> `bottom` en bas).
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

/// Dégradé horizontal (`left` à gauche -> `right` à droite).
pub fn horizontal_gradient(p: &egui::Painter, rect: Rect, left: Color32, right: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), left);
    mesh.colored_vertex(rect.right_top(), right);
    mesh.colored_vertex(rect.left_bottom(), left);
    mesh.colored_vertex(rect.right_bottom(), right);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// Assombrit les bords d'une zone (effet de torche).
pub fn vignette(p: &egui::Painter, rect: Rect, depth: f32, alpha: u8) {
    let dark = Color32::from_black_alpha(alpha);
    let clear = Color32::TRANSPARENT;
    let d = depth.min(rect.width() / 2.0).min(rect.height() / 2.0);
    vertical_gradient(p, Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + d)), dark, clear);
    vertical_gradient(p, Rect::from_min_max(pos2(rect.left(), rect.bottom() - d), rect.max), clear, dark);
    horizontal_gradient(p, Rect::from_min_max(rect.min, pos2(rect.left() + d, rect.bottom())), dark, clear);
    horizontal_gradient(p, Rect::from_min_max(pos2(rect.right() - d, rect.top()), rect.max), clear, dark);
}

/// Texte sur une ligne, tronqué avec « … ».
pub fn truncated(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.fonts(|f| f.layout_job(job))
}
