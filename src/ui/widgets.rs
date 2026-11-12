use crate::model::{Artist, Playlist, Track};
use crate::ui::anim::{ease, equalizer, lerp_c, skeleton, skeleton_circle, spinner};
use crate::ui::icons::{Ic, icon, play_icon};
use crate::ui::theme::{DIM, ELEV, GREEN, HOVER, MUTED, PILL, WHITE, bold, reg, tint};
use crate::util::{ell, mmss};
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, pos2, vec2};
use std::collections::HashSet;

pub(crate) fn text(p: &Painter, pos: Pos2, a: Align2, s: &str, f: FontId, c: Color32) -> Rect {
    p.text(pos, a, s, f, c)
}

/// Icon-only button with eased hover and press feedback. Returns the response.
pub(crate) fn icon_btn(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: Rect,
    ic: Ic,
    size: f32,
    base: Color32,
    hot: Color32,
    reduce: bool,
) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    let h = ease(ui.ctx(), id.with("h"), resp.hovered(), reduce);
    let scale = if resp.is_pointer_button_down_on() {
        0.92
    } else {
        1.0
    };
    icon(
        ui.painter(),
        ic,
        rect.center(),
        size * scale,
        lerp_c(base, hot, h),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Thin Spotify-style slider. Returns (fraction, committed).
pub(crate) fn bar(ui: &mut egui::Ui, id: egui::Id, frac: f32, w: f32, reduce: bool) -> (f32, bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 16.0), Sense::click_and_drag());
    let hot = resp.hovered() || resp.dragged();
    let h = ease(ui.ctx(), id, hot, reduce);
    let th = 4.0 + 2.0 * h;
    let track = Rect::from_center_size(r.center(), vec2(r.width(), th));
    let mut f = frac;
    if resp.dragged() || resp.clicked() || resp.drag_stopped() {
        if let Some(p) = resp.interact_pointer_pos() {
            f = ((p.x - r.left()) / r.width()).clamp(0.0, 1.0);
        }
    }
    let p = ui.painter();
    p.rect_filled(track, th / 2.0, Color32::from_rgb(77, 77, 77));
    let fill = Rect::from_min_max(
        track.min,
        pos2(track.left() + track.width() * f, track.bottom()),
    );
    p.rect_filled(fill, th / 2.0, lerp_c(WHITE, GREEN, h));
    if h > 0.01 {
        p.circle_filled(pos2(fill.right(), r.center().y), 6.0 * h, WHITE);
    }
    (f, resp.drag_stopped() || resp.clicked())
}

pub(crate) enum Evt {
    Play(usize),
    Like(usize),
    Add(usize, Pos2),
    Dl(usize),
    Remove(usize),
    Artist(usize),
}

pub(crate) struct Env<'a> {
    pub(crate) liked: &'a [Track],
    pub(crate) unavailable: &'a HashSet<String>,
    pub(crate) playing: Option<&'a Track>,
    pub(crate) live: bool,
    pub(crate) loading: bool,
    pub(crate) now: f64,
    pub(crate) reduce: bool,
}

pub(crate) fn cover(ui: &egui::Ui, t: &Track, r: Rect, radius: f32) {
    let uri = if t.thumb.is_empty() {
        format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", t.id)
    } else {
        t.thumb.clone()
    };
    let uv = if t.thumb.is_empty() {
        Rect::from_min_max(pos2(0.219, 0.0), pos2(0.781, 1.0))
    } else {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
    };
    ui.painter().rect_filled(r, radius, PILL);
    egui::Image::new(uri)
        .uv(uv)
        .corner_radius(radius)
        .paint_at(ui, r);
}

pub(crate) fn avatar(ui: &egui::Ui, a: &Artist, r: Rect) {
    ui.painter()
        .circle_filled(r.center(), r.width() / 2.0, PILL);
    if a.thumb.is_empty() {
        icon(ui.painter(), Ic::Note, r.center(), r.width() * 0.4, DIM);
    } else {
        egui::Image::new(a.thumb.clone())
            .corner_radius(r.width() / 2.0)
            .paint_at(ui, r);
    }
}

pub(crate) fn playlist_cover(ui: &egui::Ui, pl: &Playlist, r: Rect, radius: f32) {
    let p = ui.painter();
    if pl.tracks.len() >= 4 {
        let h = r.width() / 2.0;
        for (i, t) in pl.tracks.iter().take(4).enumerate() {
            let q = Rect::from_min_size(
                r.min + vec2((i % 2) as f32 * h, (i / 2) as f32 * h),
                vec2(h, h),
            );
            cover(ui, t, q, if radius > 0.0 && i == 0 { radius } else { 0.0 });
        }
    } else if let Some(t) = pl.tracks.first() {
        cover(ui, t, r, radius);
    } else {
        p.rect_filled(r, radius, tint(&pl.name, 0.45));
        icon(
            p,
            Ic::Note,
            r.center(),
            r.width() * 0.35,
            WHITE.gamma_multiply(0.85),
        );
    }
}

pub(crate) fn h2(ui: &mut egui::Ui, s: &str) {
    ui.add_space(24.0);
    ui.label(egui::RichText::new(s).font(bold(22.0)));
    ui.add_space(8.0);
}

pub(crate) fn empty_state(ui: &mut egui::Ui, ic: Ic, title: &str, body: &str) {
    ui.add_space(48.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 160.0), Sense::hover());
    let c = r.center_top() + vec2(0.0, 28.0);
    ui.painter().circle_filled(c, 28.0, PILL);
    icon(ui.painter(), ic, c, 26.0, DIM);
    text(
        ui.painter(),
        c + vec2(0.0, 52.0),
        Align2::CENTER_CENTER,
        title,
        bold(20.0),
        WHITE,
    );
    text(
        ui.painter(),
        c + vec2(0.0, 80.0),
        Align2::CENTER_CENTER,
        body,
        reg(14.0),
        DIM,
    );
}

pub(crate) fn grid_cols(w: f32) -> usize {
    (((w + 16.0) / 196.0).floor() as usize).clamp(2, 7)
}

pub(crate) enum CardKind<'a> {
    Track(&'a Track),
    Artist(&'a Artist),
    Playlist(&'a Playlist),
}

pub(crate) enum CardEvt {
    None,
    Open,
    Play,
}

/// Spotify card: hover lift, play button that slides up on hover.
pub(crate) fn card(
    ui: &mut egui::Ui,
    env: &Env,
    id: egui::Id,
    w: f32,
    kind: CardKind,
    title: &str,
    sub: &str,
) -> CardEvt {
    let (r, resp) = ui.allocate_exact_size(vec2(w, w + 62.0), Sense::click());
    let hot = resp.contains_pointer();
    let h = ease(ui.ctx(), id, hot, env.reduce);
    ui.painter().rect_filled(r, 8.0, lerp_c(ELEV, HOVER, h));
    let cv = Rect::from_min_size(r.min + vec2(12.0, 12.0), vec2(w - 24.0, w - 24.0));
    match kind {
        CardKind::Track(t) => cover(ui, t, cv, 6.0),
        CardKind::Artist(a) => avatar(ui, a, cv),
        CardKind::Playlist(p) => playlist_cover(ui, p, cv, 6.0),
    }
    let p = ui.painter();
    text(
        p,
        pos2(cv.left(), cv.bottom() + 12.0),
        Align2::LEFT_TOP,
        &ell(title, cv.width(), 15.0),
        bold(15.0),
        WHITE,
    );
    text(
        p,
        pos2(cv.left(), cv.bottom() + 34.0),
        Align2::LEFT_TOP,
        &ell(sub, cv.width(), 13.0),
        reg(13.0),
        DIM,
    );
    let pc = pos2(cv.right() - 22.0, cv.bottom() - 22.0 + (1.0 - h) * 8.0);
    let btn = Rect::from_center_size(pc, vec2(48.0, 48.0));
    let br = ui.interact(btn, id.with("play"), Sense::click());
    let press = if br.is_pointer_button_down_on() {
        0.94
    } else if br.hovered() {
        1.06
    } else {
        1.0
    };
    if h > 0.01 {
        p.circle_filled(
            pc + vec2(0.0, 3.0),
            24.0 * press,
            Color32::from_black_alpha((90.0 * h) as u8),
        );
        p.circle_filled(pc, 24.0 * press, GREEN.gamma_multiply(h));
        play_icon(
            p,
            pc + vec2(1.5, 0.0),
            10.0,
            Color32::BLACK.gamma_multiply(h),
        );
    }
    if br.clicked() {
        return CardEvt::Play;
    }
    if resp.clicked() {
        return CardEvt::Open;
    }
    CardEvt::None
}

pub(crate) fn skeleton_cards(ui: &mut egui::Ui, env: &Env, n: usize, w: f32, circle: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for _ in 0..n {
            let (r, _) = ui.allocate_exact_size(vec2(w, w + 62.0), Sense::hover());
            ui.painter().rect_filled(r, 8.0, ELEV);
            let cv = Rect::from_min_size(r.min + vec2(12.0, 12.0), vec2(w - 24.0, w - 24.0));
            if circle {
                skeleton_circle(
                    ui.painter(),
                    cv.center(),
                    cv.width() / 2.0,
                    env.now,
                    env.reduce,
                );
            } else {
                skeleton(ui.painter(), cv, 6.0, env.now, env.reduce);
            }
            skeleton(
                ui.painter(),
                Rect::from_min_size(
                    pos2(cv.left(), cv.bottom() + 12.0),
                    vec2(cv.width() * 0.7, 14.0),
                ),
                4.0,
                env.now,
                env.reduce,
            );
            skeleton(
                ui.painter(),
                Rect::from_min_size(
                    pos2(cv.left(), cv.bottom() + 34.0),
                    vec2(cv.width() * 0.45, 12.0),
                ),
                4.0,
                env.now,
                env.reduce,
            );
        }
    });
}

pub(crate) fn skeleton_rows(ui: &mut egui::Ui, env: &Env, n: usize) {
    for i in 0..n {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
        let c = r.center().y;
        let p = ui.painter();
        skeleton(
            p,
            Rect::from_center_size(pos2(r.left() + 28.0, c), vec2(14.0, 14.0)),
            3.0,
            env.now,
            env.reduce,
        );
        skeleton(
            p,
            Rect::from_min_size(pos2(r.left() + 56.0, c - 20.0), vec2(40.0, 40.0)),
            4.0,
            env.now,
            env.reduce,
        );
        skeleton(
            p,
            Rect::from_min_size(
                pos2(r.left() + 108.0, c - 16.0),
                vec2(180.0 + (i * 37 % 120) as f32, 14.0),
            ),
            4.0,
            env.now,
            env.reduce,
        );
        skeleton(
            p,
            Rect::from_min_size(pos2(r.left() + 108.0, c + 4.0), vec2(90.0, 12.0)),
            4.0,
            env.now,
            env.reduce,
        );
        skeleton(
            p,
            Rect::from_min_size(pos2(r.right() - 56.0, c - 6.0), vec2(36.0, 12.0)),
            4.0,
            env.now,
            env.reduce,
        );
    }
}

/// Track table. Returns at most one event per frame.
pub(crate) fn track_table(
    ui: &mut egui::Ui,
    env: &Env,
    list: &[Track],
    in_playlist: bool,
) -> Option<Evt> {
    let (h, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let p = ui.painter();
    text(
        p,
        h.left_center() + vec2(22.0, 0.0),
        Align2::LEFT_CENTER,
        "#",
        reg(13.0),
        MUTED,
    );
    text(
        p,
        h.left_center() + vec2(108.0, 0.0),
        Align2::LEFT_CENTER,
        "Title",
        reg(13.0),
        MUTED,
    );
    text(
        p,
        h.right_center() - vec2(16.0, 0.0),
        Align2::RIGHT_CENTER,
        "Time",
        reg(13.0),
        MUTED,
    );
    p.hline(
        h.x_range(),
        h.bottom(),
        Stroke::new(1.0, Color32::from_rgb(40, 40, 40)),
    );
    ui.add_space(8.0);
    let mut out = None;
    for (i, t) in list.iter().enumerate() {
        if let Some(e) = row(ui, env, i, t, in_playlist) {
            out = Some(e);
        }
    }
    out
}

pub(crate) fn row(
    ui: &mut egui::Ui,
    env: &Env,
    i: usize,
    t: &Track,
    in_playlist: bool,
) -> Option<Evt> {
    let id = ui.scope_id().with(("row", i, &t.id));
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::click());
    let hov = resp.contains_pointer();
    let h = ease(ui.ctx(), id, hov, env.reduce);
    let playing = env.playing == Some(t);
    let dead = env.unavailable.contains(&t.id);
    let wide = r.width() > 640.0;
    let c = r.center().y;
    let mut out = None;
    {
        let p = ui.painter();
        p.rect_filled(r, 6.0, HOVER.gamma_multiply(h));
        let num = pos2(r.left() + 28.0, c);
        if playing && env.loading {
            spinner(p, num, 7.0, GREEN, env.now, env.reduce);
        } else if hov && !dead {
            play_icon(p, num, 7.0, WHITE);
        } else if playing {
            equalizer(p, num, env.now, env.live);
        } else {
            text(
                p,
                num,
                Align2::CENTER_CENTER,
                &(i + 1).to_string(),
                reg(15.0),
                if dead { MUTED } else { DIM },
            );
        }
    }
    cover(
        ui,
        t,
        Rect::from_min_size(pos2(r.left() + 56.0, c - 20.0), vec2(40.0, 40.0)),
        4.0,
    );
    let tw = r.width() - 108.0 - if wide { 230.0 } else { 150.0 };
    let title_col = if dead {
        MUTED
    } else if playing {
        GREEN
    } else {
        WHITE
    };
    {
        let p = ui.painter();
        text(
            p,
            pos2(r.left() + 108.0, c - 2.0),
            Align2::LEFT_BOTTOM,
            &ell(&t.title, tw, 15.0),
            bold(15.0),
            title_col,
        );
        text(
            p,
            pos2(r.right() - 16.0, c),
            Align2::RIGHT_CENTER,
            &mmss(t.secs),
            reg(14.0),
            DIM,
        );
    }
    // artist link
    let sub = if dead {
        "Unavailable, no official source".to_string()
    } else {
        ell(&t.artist, tw, 13.0)
    };
    let ar = Rect::from_min_size(
        pos2(r.left() + 108.0, c + 3.0),
        vec2((sub.chars().count() as f32 * 7.2).min(tw), 18.0),
    );
    let arr = ui.interact(ar, id.with("artist"), Sense::click());
    let ah = arr.hovered() && !dead;
    text(
        ui.painter(),
        ar.left_top(),
        Align2::LEFT_TOP,
        &sub,
        reg(13.0),
        if dead {
            MUTED
        } else if ah || hov {
            WHITE
        } else {
            DIM
        },
    );
    if ah {
        ui.painter().hline(
            ar.left()..=ar.left() + sub.chars().count() as f32 * 6.4,
            ar.bottom() - 2.0,
            Stroke::new(1.0, WHITE),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if arr.clicked() {
        out = Some(Evt::Artist(i));
    }
    // actions
    let mut x = r.right() - 78.0;
    let liked = env.liked.contains(t);
    if hov || liked {
        let hr = Rect::from_center_size(pos2(x, c), vec2(32.0, 32.0));
        let hid = id.with("like");
        let resp2 = ui.interact(hr, hid, Sense::click());
        let hh = ease(ui.ctx(), hid, resp2.hovered(), env.reduce);
        let col = if liked { GREEN } else { lerp_c(DIM, WHITE, hh) };
        icon(
            ui.painter(),
            if liked { Ic::HeartFill } else { Ic::Heart },
            hr.center(),
            18.0 * if resp2.is_pointer_button_down_on() {
                0.9
            } else {
                1.0
            },
            col,
        );
        if resp2.clicked() {
            out = Some(Evt::Like(i));
        }
    }
    x -= 36.0;
    if hov && wide {
        let br = Rect::from_center_size(pos2(x, c), vec2(32.0, 32.0));
        if icon_btn(
            ui,
            id.with("add"),
            br,
            Ic::Plus,
            18.0,
            DIM,
            WHITE,
            env.reduce,
        )
        .clicked()
        {
            out = Some(Evt::Add(i, br.left_bottom() + vec2(-200.0, 6.0)));
        }
        x -= 36.0;
        let dr = Rect::from_center_size(pos2(x, c), vec2(32.0, 32.0));
        if icon_btn(
            ui,
            id.with("dl"),
            dr,
            Ic::Down,
            17.0,
            DIM,
            WHITE,
            env.reduce,
        )
        .on_hover_text("Download to ~/Music/Spotty")
        .clicked()
        {
            out = Some(Evt::Dl(i));
        }
        x -= 36.0;
        if in_playlist {
            let xr = Rect::from_center_size(pos2(x, c), vec2(32.0, 32.0));
            if icon_btn(
                ui,
                id.with("rm"),
                xr,
                Ic::Close,
                15.0,
                DIM,
                WHITE,
                env.reduce,
            )
            .on_hover_text("Remove from playlist")
            .clicked()
            {
                out = Some(Evt::Remove(i));
            }
        }
    } else if hov {
        let br = Rect::from_center_size(pos2(x, c), vec2(32.0, 32.0));
        if icon_btn(
            ui,
            id.with("add"),
            br,
            Ic::Plus,
            18.0,
            DIM,
            WHITE,
            env.reduce,
        )
        .clicked()
        {
            out = Some(Evt::Add(i, br.left_bottom()));
        }
    }
    if out.is_none() {
        if resp.double_clicked() && !dead {
            out = Some(Evt::Play(i));
        } else if resp.clicked() && !dead {
            if let Some(p) = resp.interact_pointer_pos() {
                if (p.x - (r.left() + 28.0)).abs() < 16.0 {
                    out = Some(Evt::Play(i));
                }
            }
        }
    }
    out
}
