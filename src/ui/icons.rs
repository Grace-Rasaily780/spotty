use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Ic {
    Home,
    Search,
    Library,
    Heart,
    HeartFill,
    Plus,
    Down,
    Vol,
    Mute,
    Back,
    Trash,
    Close,
    Note,
    Check,
}

pub(crate) fn arc(c: Pos2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

pub(crate) fn heart_pts(c: Pos2, r: f32) -> Vec<Pos2> {
    (0..48)
        .map(|i| {
            let t = i as f32 / 48.0 * std::f32::consts::TAU;
            let (x, f) = (
                16.0 * t.sin().powi(3),
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos(),
            );
            c + vec2(x, -f - 5.5) * (r / 17.0)
        })
        .collect()
}

pub(crate) fn icon(p: &Painter, ic: Ic, c: Pos2, s: f32, col: Color32) {
    let r = s / 2.0;
    let st = Stroke::new((s / 11.0).max(1.6), col);
    let ln = |pts: Vec<Pos2>| p.add(Shape::line(pts, st));
    match ic {
        Ic::Home => {
            ln(vec![
                c + vec2(-r, -0.05 * r),
                c + vec2(0.0, -r),
                c + vec2(r, -0.05 * r),
            ]);
            ln(vec![
                c + vec2(-0.72 * r, -0.2 * r),
                c + vec2(-0.72 * r, r),
                c + vec2(-0.2 * r, r),
                c + vec2(-0.2 * r, 0.3 * r),
                c + vec2(0.2 * r, 0.3 * r),
                c + vec2(0.2 * r, r),
                c + vec2(0.72 * r, r),
                c + vec2(0.72 * r, -0.2 * r),
            ]);
        }
        Ic::Search => {
            p.circle_stroke(c + vec2(-0.15 * r, -0.15 * r), 0.62 * r, st);
            p.line_segment(
                [c + vec2(0.3 * r, 0.3 * r), c + vec2(0.95 * r, 0.95 * r)],
                st,
            );
        }
        Ic::Library => {
            for x in [-0.8, -0.3] {
                p.rect_filled(
                    Rect::from_center_size(c + vec2(x * r, 0.0), vec2(0.32 * r, 1.9 * r)),
                    1.0,
                    col,
                );
            }
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(0.15 * r, -0.9 * r),
                    c + vec2(0.5 * r, -0.98 * r),
                    c + vec2(0.95 * r, 0.9 * r),
                    c + vec2(0.6 * r, 0.98 * r),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Ic::Heart => {
            p.add(Shape::closed_line(heart_pts(c, r), st));
        }
        Ic::HeartFill => {
            let pts = heart_pts(c, r);
            let mut m = egui::Mesh::default();
            m.colored_vertex(c + vec2(0.0, 0.0), col);
            for q in &pts {
                m.colored_vertex(*q, col);
            }
            for i in 0..pts.len() {
                m.add_triangle(0, 1 + i as u32, 1 + ((i + 1) % pts.len()) as u32);
            }
            p.add(Shape::mesh(m));
            p.add(Shape::closed_line(pts, Stroke::new(1.0, col)));
        }
        Ic::Plus => {
            p.line_segment([c + vec2(-0.8 * r, 0.0), c + vec2(0.8 * r, 0.0)], st);
            p.line_segment([c + vec2(0.0, -0.8 * r), c + vec2(0.0, 0.8 * r)], st);
        }
        Ic::Down => {
            p.line_segment([c + vec2(0.0, -0.9 * r), c + vec2(0.0, 0.45 * r)], st);
            ln(vec![
                c + vec2(-0.5 * r, 0.0),
                c + vec2(0.0, 0.55 * r),
                c + vec2(0.5 * r, 0.0),
            ]);
            p.line_segment(
                [c + vec2(-0.9 * r, 0.95 * r), c + vec2(0.9 * r, 0.95 * r)],
                st,
            );
        }
        Ic::Vol | Ic::Mute => {
            p.add(Shape::closed_line(
                vec![
                    c + vec2(-r, -0.32 * r),
                    c + vec2(-0.45 * r, -0.32 * r),
                    c + vec2(0.1 * r, -0.85 * r),
                    c + vec2(0.1 * r, 0.85 * r),
                    c + vec2(-0.45 * r, 0.32 * r),
                    c + vec2(-r, 0.32 * r),
                ],
                st,
            ));
            if ic == Ic::Vol {
                ln(arc(c + vec2(0.1 * r, 0.0), 0.55 * r, -0.8, 0.8, 8));
                ln(arc(c + vec2(0.1 * r, 0.0), 0.95 * r, -0.8, 0.8, 10));
            } else {
                p.line_segment(
                    [c + vec2(0.45 * r, -0.4 * r), c + vec2(1.0 * r, 0.4 * r)],
                    st,
                );
                p.line_segment(
                    [c + vec2(0.45 * r, 0.4 * r), c + vec2(1.0 * r, -0.4 * r)],
                    st,
                );
            }
        }
        Ic::Back => {
            ln(vec![
                c + vec2(0.3 * r, -0.85 * r),
                c + vec2(-0.45 * r, 0.0),
                c + vec2(0.3 * r, 0.85 * r),
            ]);
        }
        Ic::Trash => {
            p.line_segment(
                [c + vec2(-0.9 * r, -0.5 * r), c + vec2(0.9 * r, -0.5 * r)],
                st,
            );
            ln(vec![
                c + vec2(-0.25 * r, -0.5 * r),
                c + vec2(-0.25 * r, -0.9 * r),
                c + vec2(0.25 * r, -0.9 * r),
                c + vec2(0.25 * r, -0.5 * r),
            ]);
            ln(vec![
                c + vec2(-0.6 * r, -0.4 * r),
                c + vec2(-0.5 * r, 0.95 * r),
                c + vec2(0.5 * r, 0.95 * r),
                c + vec2(0.6 * r, -0.4 * r),
            ]);
        }
        Ic::Close => {
            p.line_segment(
                [c + vec2(-0.7 * r, -0.7 * r), c + vec2(0.7 * r, 0.7 * r)],
                st,
            );
            p.line_segment(
                [c + vec2(-0.7 * r, 0.7 * r), c + vec2(0.7 * r, -0.7 * r)],
                st,
            );
        }
        Ic::Note => {
            p.circle_filled(c + vec2(-0.4 * r, 0.65 * r), 0.42 * r, col);
            ln(vec![
                c + vec2(0.0, 0.65 * r),
                c + vec2(0.0, -0.85 * r),
                c + vec2(0.7 * r, -0.45 * r),
            ]);
        }
        Ic::Check => {
            ln(vec![
                c + vec2(-0.8 * r, 0.05 * r),
                c + vec2(-0.25 * r, 0.6 * r),
                c + vec2(0.85 * r, -0.6 * r),
            ]);
        }
    }
}

pub(crate) fn play_icon(p: &Painter, c: Pos2, r: f32, col: Color32) {
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(-r * 0.7, -r),
            c + vec2(-r * 0.7, r),
            c + vec2(r, 0.0),
        ],
        col,
        Stroke::NONE,
    ));
}

pub(crate) fn pause_icon(p: &Painter, c: Pos2, r: f32, col: Color32) {
    for dx in [-r * 0.55, r * 0.55] {
        p.rect_filled(
            Rect::from_center_size(c + vec2(dx, 0.0), vec2(r * 0.6, r * 1.8)),
            1.0,
            col,
        );
    }
}

pub(crate) fn skip_icon(p: &Painter, c: Pos2, r: f32, col: Color32, back: bool) {
    let d = if back { -1.0 } else { 1.0 };
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(-d * r * 0.9, -r),
            c + vec2(-d * r * 0.9, r),
            c + vec2(d * r * 0.7, 0.0),
        ],
        col,
        Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_center_size(c + vec2(d * r, 0.0), vec2(r * 0.35, r * 2.0)),
        1.0,
        col,
    );
}
