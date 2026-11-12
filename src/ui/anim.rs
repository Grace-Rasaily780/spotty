use crate::ui::icons::arc;
use crate::ui::theme::{GREEN, PILL};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

/// Eased 0..1 hover/press value (cubic-out, 120 ms). Snaps when reduced motion is on.
pub(crate) fn ease(ctx: &egui::Context, id: egui::Id, on: bool, reduce: bool) -> f32 {
    if reduce {
        on as u8 as f32
    } else {
        ctx.animate_bool_with_time_and_easing(id, on, 0.12, egui::emath::easing::cubic_out)
    }
}

pub(crate) fn lerp_c(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// Skeleton block: base fill + soft light sweep (static when reduced motion).
pub(crate) fn skeleton(p: &Painter, r: Rect, radius: f32, now: f64, reduce: bool) {
    p.rect_filled(r, radius, PILL);
    if reduce || r.width() < 8.0 {
        return;
    }
    let w = r.width();
    let x = r.left() + ((now * 0.9 % 1.6) as f32 - 0.3) * w;
    let band = w * 0.35;
    let (x0, x1, xm) = (
        (x - band).max(r.left()),
        (x + band).min(r.right()),
        x.clamp(r.left(), r.right()),
    );
    if x1 <= x0 {
        return;
    }
    let mut m = egui::Mesh::default();
    let (a, b) = (Color32::from_white_alpha(0), Color32::from_white_alpha(16));
    for (px, c) in [(x0, a), (xm, b), (x1, a)] {
        m.colored_vertex(pos2(px, r.top()), c);
        m.colored_vertex(pos2(px, r.bottom()), c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(1, 2, 3);
    m.add_triangle(2, 3, 4);
    m.add_triangle(3, 4, 5);
    p.with_clip_rect(r).add(Shape::mesh(m));
}

pub(crate) fn skeleton_circle(p: &Painter, c: Pos2, r: f32, now: f64, reduce: bool) {
    let t = if reduce {
        0.0
    } else {
        (0.5 + 0.5 * (now * 3.0).sin()) as f32
    };
    p.circle_filled(c, r, lerp_c(PILL, Color32::from_rgb(50, 50, 50), t));
}

pub(crate) fn spinner(p: &Painter, c: Pos2, r: f32, col: Color32, now: f64, reduce: bool) {
    p.circle_stroke(c, r, Stroke::new(2.0, col.gamma_multiply(0.25)));
    let a0 = if reduce { 0.0 } else { (now * 5.0) as f32 };
    p.add(Shape::line(
        arc(c, r, a0, a0 + 1.6, 14),
        Stroke::new(2.0, col),
    ));
}

pub(crate) fn equalizer(p: &Painter, c: Pos2, now: f64, live: bool) {
    for (i, ph) in [0.0f64, 1.7, 3.1].iter().enumerate() {
        let h = if live {
            4.0 + 8.0 * ((now * (5.0 + i as f64) + ph).sin().abs() as f32)
        } else {
            4.0
        };
        p.rect_filled(
            Rect::from_min_max(
                pos2(c.x - 7.0 + i as f32 * 5.5, c.y + 6.0 - h),
                pos2(c.x - 7.0 + i as f32 * 5.5 + 3.5, c.y + 6.0),
            ),
            1.0,
            GREEN,
        );
    }
}
