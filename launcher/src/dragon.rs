//! Petit dragon rouge et or qui traverse la fenêtre de temps en temps
//! (ailes qui battent, queue qui ondule, bouffée de flammes à mi-parcours).
//! Purement décoratif : il ne capte aucun clic.

use crate::theme::{GOLD, GOLD_BRIGHT, GOLD_DIM};
use eframe::egui::{self, pos2, vec2, Color32, Painter, Pos2, Shape, Stroke};
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

/// Première apparition après le lancement (s).
const FIRST: f32 = 2.5;
/// Une traversée toutes les … (s).
const EVERY: f32 = 45.0;
/// Durée d'une traversée (s).
const FLIGHT: f32 = 9.0;
/// Taille (1.0 ≈ 80 px du museau au bout de la queue).
const SCALE: f32 = 1.3;

const BODY: Color32 = Color32::from_rgb(0x8E, 0x1C, 0x1C);
const DARK: Color32 = Color32::from_rgb(0x5A, 0x10, 0x10);

pub struct Dragon {
    start: Instant,
}

impl Dragon {
    pub fn new() -> Self {
        Dragon { start: Instant::now() }
    }

    /// Dessine le dragon s'il est en vol. `hidden` : une fenêtre est ouverte, il ne passe pas.
    pub fn show(&self, ctx: &egui::Context, hidden: bool) {
        let t = self.start.elapsed().as_secs_f32() - FIRST;
        if t < 0.0 {
            ctx.request_repaint_after(Duration::from_secs_f32(-t));
            return;
        }
        let pass = (t / EVERY).floor() as usize;
        let local = t - pass as f32 * EVERY;
        if local > FLIGHT {
            ctx.request_repaint_after(Duration::from_secs_f32(EVERY - local));
            return;
        }
        ctx.request_repaint();
        if hidden {
            return;
        }

        // Alterne gauche→droite / droite→gauche, à une hauteur différente à chaque passage.
        let k = local / FLIGHT;
        let dir = if pass % 2 == 0 { 1.0 } else { -1.0 };
        let screen = ctx.screen_rect();
        let (x0, x1) = (screen.left() - 110.0, screen.right() + 110.0);
        let x = if dir > 0.0 { x0 + (x1 - x0) * k } else { x1 - (x1 - x0) * k };
        let lanes = [0.32, 0.55, 0.22, 0.45];
        let y = screen.top() + screen.height() * lanes[pass % lanes.len()] + (k * TAU * 1.5).sin() * 22.0;

        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dragon")));
        draw(&painter, pos2(x, y), dir, local, k);
    }
}

/// Polygone convexe ; l'ordre des points est rétabli quand le dragon est retourné.
fn poly(p: &Painter, mut pts: Vec<Pos2>, dir: f32, fill: Color32, stroke: Stroke) {
    if dir < 0.0 {
        pts.reverse();
    }
    p.add(Shape::convex_polygon(pts, fill, stroke));
}

fn draw(p: &Painter, at: Pos2, dir: f32, time: f32, k: f32) {
    let s = SCALE;
    let pt = |x: f32, y: f32| at + vec2(x * dir * s, y * s);
    let rim = Stroke::new(1.2_f32, GOLD);
    let flap = (time * 7.0).sin();

    // Aile lointaine, plus sombre et en léger décalage.
    wing(p, &pt, dir, (time * 7.0 - 0.5).sin() * 0.85, DARK, Stroke::new(1.0_f32, GOLD_DIM), 4.0);

    // Queue : segments de plus en plus fins qui ondulent.
    let n = 16;
    let mut prev = pt(-12.0, 1.0);
    let (mut lx, mut ly) = (0.0, 0.0);
    for i in 1..=n {
        let f = i as f32 / n as f32;
        lx = -12.0 - f * 36.0;
        ly = 1.0 + (time * 5.0 - f * 4.5).sin() * 6.0 * f + f * 3.0;
        let cur = pt(lx, ly);
        p.line_segment([prev, cur], Stroke::new((1.0 - f) * 6.0 * s + 1.2, BODY));
        prev = cur;
    }
    poly(p, vec![pt(lx + 1.0, ly - 4.0), pt(lx - 7.0, ly), pt(lx + 1.0, ly + 4.0)], dir, GOLD, Stroke::NONE);

    // Pattes repliées.
    for (x, dx) in [(7.0, 3.0), (-6.0, 2.0)] {
        p.line_segment([pt(x, 4.0), pt(x + dx, 10.0)], Stroke::new(2.2 * s, DARK));
        p.line_segment([pt(x + dx, 10.0), pt(x + dx + 3.0, 10.5)], Stroke::new(1.4 * s, GOLD_DIM));
    }

    // Corps.
    let body: Vec<Pos2> = (0..24).map(|i| {
        let a = i as f32 / 24.0 * TAU;
        pt(a.cos() * 15.0, a.sin() * 7.0)
    }).collect();
    poly(p, body, dir, BODY, rim);
    // Ventre doré.
    p.line_segment([pt(-9.0, 4.5), pt(10.0, 4.0)], Stroke::new(1.6 * s, GOLD_DIM));

    // Cou et tête.
    p.line_segment([pt(9.0, -3.0), pt(20.0, -9.0)], Stroke::new(7.0 * s, BODY));
    poly(
        p,
        vec![pt(16.0, -10.0), pt(19.0, -14.5), pt(26.0, -14.0), pt(34.0, -9.0), pt(33.0, -6.0), pt(21.0, -5.0)],
        dir,
        BODY,
        rim,
    );
    p.line_segment([pt(19.0, -14.0), pt(12.0, -21.0)], Stroke::new(1.8 * s, GOLD));
    p.line_segment([pt(22.5, -14.5), pt(17.5, -22.0)], Stroke::new(1.8 * s, GOLD));
    p.circle_filled(pt(26.0, -11.0), 1.5 * s, GOLD_BRIGHT);
    p.circle_filled(pt(32.0, -8.0), 0.8 * s, DARK);

    // Aile proche.
    wing(p, &pt, dir, flap, BODY, rim, 0.0);

    // Bouffée de flammes au milieu de la traversée.
    if (0.42..0.62).contains(&k) {
        let f = (k - 0.42) / 0.2;
        for j in 0..7 {
            let jf = j as f32;
            let c = pt(36.0 + jf * 5.0 + f * 8.0, -7.5 + (time * 22.0 + jf).sin() * 1.6 + jf * 0.6);
            let alpha = ((1.0 - f * 0.6) * (220.0 - jf * 28.0)).max(0.0) as u8;
            let color = Color32::from_rgba_unmultiplied(255, (120.0 + jf * 18.0) as u8, 30, alpha);
            p.circle_filled(c, (1.6 + jf * 1.1) * s, color);
        }
    }
}

/// Aile membraneuse : deux triangles depuis l'épaule ; `up` va de -1 (aile basse) à 1 (aile haute).
fn wing(p: &Painter, pt: &impl Fn(f32, f32) -> Pos2, dir: f32, up: f32, fill: Color32, stroke: Stroke, dx: f32) {
    let shoulder = pt(3.0 + dx, -5.0);
    let tip = pt(-7.0 + dx, -5.0 - 26.0 * up);
    let mid = pt(-17.0 + dx, -5.0 - 12.0 * up);
    let back = pt(-9.0 + dx, -2.0);
    poly(p, vec![shoulder, tip, mid], dir * if up < 0.0 { -1.0 } else { 1.0 }, fill, stroke);
    poly(p, vec![shoulder, mid, back], dir * if up < 0.0 { -1.0 } else { 1.0 }, fill, stroke);
    p.line_segment([shoulder, tip], Stroke::new(1.6_f32, GOLD));
}
