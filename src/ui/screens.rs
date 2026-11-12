use crate::db;
use crate::model::{Playlist, Track, View};
use crate::ui::App;
use crate::ui::anim::{ease, lerp_c, skeleton, skeleton_circle, spinner};
use crate::ui::icons::{Ic, icon, pause_icon, play_icon, skip_icon};
use crate::ui::theme::{BASE, BG, DIM, ELEV, GREEN, HOVER, MUTED, PILL, WHITE, bold, reg, tint};
use crate::ui::widgets::{
    CardEvt, CardKind, avatar, bar, card, cover, empty_state, grid_cols, h2, icon_btn,
    playlist_cover, skeleton_cards, skeleton_rows, text, track_table,
};
use crate::util::{ell, mmss, short_num};
use eframe::egui::{self, Align2, Color32, Rect, Sense, Shape, Stroke, pos2, vec2};
use std::time::Duration;

impl App {
    pub(crate) fn player_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::bottom("player")
            .exact_size(92.0)
            .frame(
                egui::Frame::NONE
                    .fill(BASE)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                let cur = self.cur.and_then(|c| self.queue.get(c)).cloned();
                let reduce = self.reduce;
                egui_extras::StripBuilder::new(ui)
                    .size(egui_extras::Size::relative(0.3))
                    .size(egui_extras::Size::relative(0.4))
                    .size(egui_extras::Size::remainder())
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            if let Some(t) = &cur {
                                let (r, _) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 64.0),
                                    Sense::hover(),
                                );
                                cover(
                                    ui,
                                    t,
                                    Rect::from_min_size(r.min + vec2(0.0, 4.0), vec2(56.0, 56.0)),
                                    4.0,
                                );
                                let w = r.width() - 68.0 - 40.0;
                                text(
                                    ui.painter(),
                                    r.min + vec2(68.0, 30.0),
                                    Align2::LEFT_BOTTOM,
                                    &ell(&t.title, w, 14.0),
                                    bold(14.0),
                                    WHITE,
                                );
                                text(
                                    ui.painter(),
                                    r.min + vec2(68.0, 34.0),
                                    Align2::LEFT_TOP,
                                    &ell(
                                        &format!(
                                            "{}{}",
                                            t.artist,
                                            if self.preview {
                                                "  •  30 s preview"
                                            } else {
                                                ""
                                            }
                                        ),
                                        w,
                                        12.0,
                                    ),
                                    reg(12.0),
                                    DIM,
                                );
                                let liked = self.liked.contains(t);
                                let hr = Rect::from_center_size(
                                    pos2(r.right() - 16.0, r.center().y),
                                    vec2(32.0, 32.0),
                                );
                                if icon_btn(
                                    ui,
                                    ui.scope_id().with("plike"),
                                    hr,
                                    if liked { Ic::HeartFill } else { Ic::Heart },
                                    18.0,
                                    if liked { GREEN } else { DIM },
                                    if liked { GREEN } else { WHITE },
                                    reduce,
                                )
                                .clicked()
                                {
                                    self.toggle_like(t);
                                }
                            }
                        });
                        strip.cell(|ui| {
                            ui.add_space(4.0);
                            let busy = self.loading;
                            let paused = self.player.is_paused() || self.player.empty();
                            let mut act: Option<u8> = None;
                            let (row, _) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 38.0),
                                Sense::hover(),
                            );
                            let cx = row.center().x;
                            let mid = pos2(cx, row.center().y);
                            let pb = Rect::from_center_size(mid, vec2(38.0, 38.0));
                            let (prev, next) = (
                                Rect::from_center_size(mid - vec2(56.0, 0.0), vec2(32.0, 32.0)),
                                Rect::from_center_size(mid + vec2(56.0, 0.0), vec2(32.0, 32.0)),
                            );
                            for (rc, back, id) in [(prev, true, "prev"), (next, false, "next")] {
                                let rs = ui.interact(rc, ui.scope_id().with(id), Sense::click());
                                let hh =
                                    ease(ctx, ui.scope_id().with((id, "h")), rs.hovered(), reduce);
                                skip_icon(
                                    ui.painter(),
                                    rc.center(),
                                    7.0 * if rs.is_pointer_button_down_on() {
                                        0.9
                                    } else {
                                        1.0
                                    },
                                    lerp_c(DIM, WHITE, hh),
                                    back,
                                );
                                if rs.clicked() {
                                    act = Some(back as u8);
                                }
                            }
                            let rs = ui.interact(pb, ui.scope_id().with("pp"), Sense::click());
                            let hh = ease(ctx, ui.scope_id().with("pph"), rs.hovered(), reduce);
                            let sc = if rs.is_pointer_button_down_on() {
                                0.92
                            } else {
                                1.0 + 0.06 * hh
                            };
                            ui.painter().circle_filled(mid, 19.0 * sc, WHITE);
                            if busy {
                                spinner(ui.painter(), mid, 8.0, Color32::BLACK, self.now, reduce);
                            } else if paused {
                                play_icon(ui.painter(), mid + vec2(1.0, 0.0), 8.0, Color32::BLACK);
                            } else {
                                pause_icon(ui.painter(), mid, 8.0, Color32::BLACK);
                            }
                            if rs.clicked() && !busy {
                                act = Some(2);
                            }
                            match act {
                                Some(1) => {
                                    if let Some(c) = self.cur.filter(|&c| c > 0) {
                                        self.go(ctx, c - 1)
                                    }
                                }
                                Some(0) => {
                                    if let Some(c) = self.cur.filter(|c| c + 1 < self.queue.len()) {
                                        self.go(ctx, c + 1)
                                    }
                                }
                                Some(2) => {
                                    if self.started {
                                        if self.player.is_paused() {
                                            self.player.play()
                                        } else {
                                            self.player.pause()
                                        }
                                    }
                                }
                                _ => {}
                            }
                            let total = if self.preview {
                                30.0
                            } else {
                                cur.as_ref().map_or(1.0, |t| t.secs.max(1) as f32)
                            };
                            if !self.dragging {
                                self.seek = self.player.get_pos().as_secs_f32().min(total);
                            }
                            ui.horizontal(|ui| {
                                let w = ui.available_width();
                                ui.add_space(0.0);
                                ui.label(
                                    egui::RichText::new(mmss(self.seek as u64))
                                        .font(reg(11.0))
                                        .color(DIM),
                                );
                                let (f, done) = bar(
                                    ui,
                                    ui.scope_id().with("seek"),
                                    self.seek / total,
                                    (w - 100.0).max(40.0),
                                    reduce,
                                );
                                if self.started {
                                    if done {
                                        self.seek = f * total;
                                        let _ = self
                                            .player
                                            .try_seek(Duration::from_secs_f32(self.seek));
                                        self.dragging = false;
                                    } else if (f * total - self.seek).abs() > 0.5 {
                                        self.seek = f * total;
                                        self.dragging = true;
                                    }
                                }
                                ui.label(
                                    egui::RichText::new(mmss(total as u64))
                                        .font(reg(11.0))
                                        .color(DIM),
                                );
                            });
                        });
                        strip.cell(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let cur_v = self.player.volume();
                                    let (v, _) =
                                        bar(ui, ui.scope_id().with("vol"), cur_v, 100.0, reduce);
                                    if (v - cur_v).abs() > 0.001 {
                                        self.player.set_volume(v);
                                        self.muted_vol = None;
                                    }
                                    let (rc, _) =
                                        ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                                    let muted = cur_v < 0.01;
                                    if icon_btn(
                                        ui,
                                        ui.scope_id().with("mute"),
                                        rc,
                                        if muted { Ic::Mute } else { Ic::Vol },
                                        18.0,
                                        DIM,
                                        WHITE,
                                        reduce,
                                    )
                                    .clicked()
                                    {
                                        if muted {
                                            self.player
                                                .set_volume(self.muted_vol.take().unwrap_or(1.0));
                                        } else {
                                            self.muted_vol = Some(cur_v);
                                            self.player.set_volume(0.0);
                                        }
                                    }
                                },
                            );
                        });
                    });
            });
    }

    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, narrow: bool) {
        let w = if narrow { 88.0 } else { 320.0 };
        let reduce = self.reduce;
        egui::Panel::left("side")
            .exact_size(w)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(BASE).inner_margin(egui::Margin {
                left: 8,
                right: 0,
                top: 8,
                bottom: 0,
            }))
            .show(ui, |ui| {
                let card = egui::Frame::new()
                    .fill(BG)
                    .corner_radius(8.0)
                    .inner_margin(if narrow { 8.0 } else { 12.0 });
                card.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (v, ic, label) in [
                        (View::Home, Ic::Home, "Home"),
                        (View::Search, Ic::Search, "Search"),
                    ] {
                        let (r, resp) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                        let on = self.view == v;
                        let hh = ease(ctx, ui.scope_id().with(label), resp.hovered(), reduce);
                        let col = if on { WHITE } else { lerp_c(DIM, WHITE, hh) };
                        let ic_pos = if narrow {
                            r.center()
                        } else {
                            pos2(r.left() + 24.0, r.center().y)
                        };
                        icon(ui.painter(), ic, ic_pos, 22.0, col);
                        if !narrow {
                            text(
                                ui.painter(),
                                pos2(r.left() + 56.0, r.center().y),
                                Align2::LEFT_CENTER,
                                label,
                                bold(16.0),
                                col,
                            );
                        }
                        if resp.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            if v == View::Search {
                                self.focus_search = true;
                            }
                            self.nav(v);
                        }
                    }
                });
                ui.add_space(8.0);
                egui::Frame::new()
                    .fill(BG)
                    .corner_radius(8.0)
                    .inner_margin(if narrow { 8.0 } else { 12.0 })
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        let (hr, _) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
                        let plus = Rect::from_center_size(
                            if narrow {
                                hr.center()
                            } else {
                                pos2(hr.right() - 20.0, hr.center().y)
                            },
                            vec2(36.0, 36.0),
                        );
                        if !narrow {
                            icon(
                                ui.painter(),
                                Ic::Library,
                                pos2(hr.left() + 24.0, hr.center().y),
                                22.0,
                                DIM,
                            );
                            text(
                                ui.painter(),
                                pos2(hr.left() + 56.0, hr.center().y),
                                Align2::LEFT_CENTER,
                                "Your Library",
                                bold(16.0),
                                DIM,
                            );
                        }
                        if icon_btn(
                            ui,
                            ui.scope_id().with("newpl"),
                            plus,
                            Ic::Plus,
                            20.0,
                            DIM,
                            WHITE,
                            reduce,
                        )
                        .on_hover_text("Create playlist")
                        .clicked()
                        {
                            let i = self.new_playlist(None);
                            self.nav(View::Playlist(i));
                            self.toast("Playlist created. Click its name to rename.");
                        }
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                let mut go = None;
                                let liked_n = self.liked.len();
                                let item = |ui: &mut egui::Ui,
                                            sel: bool,
                                            id: &str,
                                            kind: u8,
                                            pl: Option<&Playlist>,
                                            title: &str,
                                            sub: String|
                                 -> bool {
                                    let (r, resp) = ui.allocate_exact_size(
                                        vec2(ui.available_width(), 64.0),
                                        Sense::click(),
                                    );
                                    let hh =
                                        ease(ctx, ui.scope_id().with(id), resp.hovered(), reduce);
                                    ui.painter().rect_filled(
                                        r,
                                        6.0,
                                        if sel { HOVER } else { ELEV.gamma_multiply(hh) }
                                            .gamma_multiply(if sel { 1.0 } else { hh }),
                                    );
                                    let sq = Rect::from_min_size(
                                        r.min
                                            + vec2(
                                                if narrow {
                                                    (r.width() - 48.0) / 2.0
                                                } else {
                                                    8.0
                                                },
                                                8.0,
                                            ),
                                        vec2(48.0, 48.0),
                                    );
                                    if kind == 0 {
                                        ui.painter().rect_filled(
                                            sq,
                                            4.0,
                                            Color32::from_rgb(80, 56, 220),
                                        );
                                        icon(ui.painter(), Ic::HeartFill, sq.center(), 22.0, WHITE);
                                    } else if let Some(p) = pl {
                                        playlist_cover(ui, p, sq, 4.0);
                                    }
                                    if !narrow {
                                        text(
                                            ui.painter(),
                                            r.min + vec2(68.0, 30.0),
                                            Align2::LEFT_BOTTOM,
                                            &ell(title, r.width() - 80.0, 15.0),
                                            bold(15.0),
                                            WHITE,
                                        );
                                        text(
                                            ui.painter(),
                                            r.min + vec2(68.0, 34.0),
                                            Align2::LEFT_TOP,
                                            &sub,
                                            reg(13.0),
                                            DIM,
                                        );
                                    }
                                    if resp.hovered() {
                                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    resp.clicked()
                                };
                                if item(
                                    ui,
                                    self.view == View::Liked,
                                    "liked",
                                    0,
                                    None,
                                    "Liked Songs",
                                    format!("Playlist • {liked_n} songs"),
                                ) {
                                    go = Some(View::Liked);
                                }
                                for (i, p) in self.playlists.iter().enumerate() {
                                    if item(
                                        ui,
                                        self.view == View::Playlist(i),
                                        &format!("pl{i}"),
                                        1,
                                        Some(p),
                                        &p.name,
                                        format!("Playlist • {} songs", p.tracks.len()),
                                    ) {
                                        go = Some(View::Playlist(i));
                                    }
                                }
                                if let Some(v) = go {
                                    self.nav(v);
                                }
                            });
                    });
            });
    }

    pub(crate) fn main_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BASE).inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 8,
                bottom: 0,
            }))
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(BG)
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        // gradient wash behind the header, per view
                        let top = ui.max_rect();
                        let tone = match &self.view {
                            View::Liked => Color32::from_rgb(70, 50, 150),
                            View::Home => Color32::from_rgb(30, 78, 58),
                            View::Search => Color32::from_rgb(44, 44, 44),
                            View::Artist => self
                                .artist
                                .as_ref()
                                .map_or(Color32::from_rgb(44, 44, 44), |a| tint(&a.name, 0.38)),
                            View::Playlist(i) => {
                                self.playlists.get(*i).map_or(BG, |p| tint(&p.name, 0.4))
                            }
                        };
                        let g = Rect::from_min_size(top.min, vec2(top.width(), 320.0));
                        let mut m = egui::Mesh::default();
                        for (pt, c) in [
                            (g.left_top(), tone),
                            (g.right_top(), tone),
                            (g.right_bottom(), BG),
                            (g.left_bottom(), BG),
                        ] {
                            m.colored_vertex(pt, c);
                        }
                        m.add_triangle(0, 1, 2);
                        m.add_triangle(0, 2, 3);
                        ui.painter().add(Shape::mesh(m));

                        egui::Frame::NONE
                            .inner_margin(egui::Margin::symmetric(24, 16))
                            .show(ui, |ui| {
                                ui.set_min_size(ui.available_size());
                                self.top_bar(ui, ctx);
                                ui.add_space(12.0);
                                self.setup_banner(ui, ctx);
                                // fade content in on navigation (reduced motion: instant)
                                let a = if self.reduce {
                                    1.0
                                } else {
                                    egui::emath::easing::cubic_out(
                                        ((self.now - self.nav_t) / 0.22).clamp(0.0, 1.0) as f32,
                                    )
                                };
                                ui.set_opacity(0.35 + 0.65 * a);
                                egui::ScrollArea::vertical()
                                    .auto_shrink(false)
                                    .show(ui, |ui| {
                                        match self.view.clone() {
                                            View::Home => self.home(ui, ctx),
                                            View::Search => self.search_view(ui, ctx),
                                            View::Artist => self.artist_view(ui, ctx),
                                            View::Liked => {
                                                let l = self.liked.clone();
                                                self.collection(ui, ctx, None, &l)
                                            }
                                            View::Playlist(i) => {
                                                let l = self
                                                    .playlists
                                                    .get(i)
                                                    .map(|p| p.tracks.clone())
                                                    .unwrap_or_default();
                                                self.collection(ui, ctx, Some(i), &l)
                                            }
                                        }
                                        ui.add_space(32.0);
                                    });
                            });
                    });
            });
    }

    /// "Official only" filter chip next to the search box (remembered between launches).
    pub(crate) fn official_chip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (r, resp) = ui.allocate_exact_size(vec2(140.0, 40.0), Sense::click());
        let on = self.official_only;
        let h = ease(
            ctx,
            ui.scope_id().with("official-chip"),
            resp.hovered(),
            self.reduce,
        );
        let fill = if on {
            GREEN.gamma_multiply(0.16 + 0.08 * h)
        } else {
            lerp_c(PILL, HOVER, h)
        };
        ui.painter().rect_filled(r, 20.0, fill);
        let c = pos2(r.left() + 22.0, r.center().y);
        if on {
            icon(ui.painter(), Ic::Check, c, 14.0, GREEN);
        } else {
            ui.painter().circle_stroke(c, 6.0, Stroke::new(1.6, DIM));
        }
        text(
            ui.painter(),
            pos2(r.left() + 40.0, r.center().y),
            Align2::LEFT_CENTER,
            "Official only",
            bold(13.0),
            if on { GREEN } else { DIM },
        );
        if resp.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            self.official_only = !on;
            db::put_setting("official_only", if self.official_only { "1" } else { "0" });
            if self.searched {
                self.search(ctx);
            }
        }
    }

    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let reduce = self.reduce;
        ui.horizontal(|ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
            let can = !self.back.is_empty();
            let hh = ease(
                ctx,
                ui.scope_id().with("backh"),
                resp.hovered() && can,
                reduce,
            );
            ui.painter().circle_filled(
                r.center(),
                20.0,
                Color32::from_black_alpha(130).gamma_multiply(1.0 + hh * 0.4),
            );
            icon(
                ui.painter(),
                Ic::Back,
                r.center(),
                18.0,
                if can { WHITE } else { MUTED },
            );
            if resp.clicked() && can {
                self.go_back();
            }
            ui.add_space(4.0);
            let fr = egui::Frame::new()
                .fill(PILL)
                .corner_radius(24.0)
                .inner_margin(egui::Margin::symmetric(16, 10));
            let sid = egui::Id::unique("search-box");
            let focused = ctx.memory(|m| m.has_focus(sid));
            fr.stroke(if focused {
                Stroke::new(2.0, WHITE)
            } else {
                Stroke::NONE
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (ir, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                    icon(
                        ui.painter(),
                        Ic::Search,
                        ir.center(),
                        20.0,
                        if focused { WHITE } else { DIM },
                    );
                    let w = (ui.available_width() - 40.0).clamp(160.0, 380.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id(sid)
                            .frame(egui::Frame::NONE)
                            .hint_text(
                                egui::RichText::new("What do you want to play?").color(MUTED),
                            )
                            .desired_width(w)
                            .font(reg(15.0)),
                    );
                    if self.focus_search {
                        r.request_focus();
                        self.focus_search = false;
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.search(ctx);
                    }
                    if !self.query.is_empty() {
                        let (cr, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                        if icon_btn(
                            ui,
                            ui.scope_id().with("clr"),
                            cr,
                            Ic::Close,
                            14.0,
                            DIM,
                            WHITE,
                            reduce,
                        )
                        .clicked()
                        {
                            self.query.clear();
                            r.request_focus();
                        }
                    }
                });
            });
            ui.add_space(8.0);
            self.official_chip(ui, ctx);
        });
    }

    // ── views ──
    pub(crate) fn home(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(egui::RichText::new(&self.greeting).font(bold(32.0)));
        let cols = grid_cols(ui.available_width());
        let w = (ui.available_width() - 16.0 * (cols as f32 - 1.0)) / cols as f32;
        let mut ev: Option<(u8, usize, bool)> = None; // (shelf, idx, play)
        {
            let env = self.env();
            if self.recent.is_empty() && self.playlists.is_empty() && self.liked.is_empty() {
                empty_state(
                    ui,
                    Ic::Note,
                    "Your music starts here",
                    "Search for an artist or song, then press play.",
                );
                let (r, resp) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                let b = Rect::from_center_size(r.center(), vec2(150.0, 44.0));
                let hh = ease(ctx, ui.scope_id().with("cta"), resp.hovered(), env.reduce);
                ui.painter().rect_filled(b.expand(2.0 * hh), 22.0, WHITE);
                text(
                    ui.painter(),
                    b.center(),
                    Align2::CENTER_CENTER,
                    "Find music",
                    bold(15.0),
                    Color32::BLACK,
                );
                if resp.clicked() {
                    ev = Some((9, 0, false));
                }
            }
            if !self.recent.is_empty() {
                h2(ui, "Recently played");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    for (i, t) in self.recent.iter().take(cols).enumerate() {
                        match card(
                            ui,
                            &env,
                            ui.scope_id().with(("rc", i)),
                            w,
                            CardKind::Track(t),
                            &t.title,
                            &t.artist,
                        ) {
                            CardEvt::Play | CardEvt::Open => ev = Some((0, i, true)),
                            CardEvt::None => {}
                        }
                    }
                });
            }
            if !self.playlists.is_empty() {
                h2(ui, "Your playlists");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(16.0, 16.0);
                    for (i, p) in self.playlists.iter().enumerate() {
                        match card(
                            ui,
                            &env,
                            ui.scope_id().with(("pc", i)),
                            w,
                            CardKind::Playlist(p),
                            &p.name,
                            &format!("{} songs", p.tracks.len()),
                        ) {
                            CardEvt::Play => ev = Some((1, i, true)),
                            CardEvt::Open => ev = Some((1, i, false)),
                            CardEvt::None => {}
                        }
                    }
                });
            }
        }
        match ev {
            Some((0, i, _)) => {
                let l = self.recent.clone();
                self.play(ctx, l, i)
            }
            Some((1, i, play)) => {
                if play {
                    let l = self.playlists[i].tracks.clone();
                    if !l.is_empty() {
                        self.play(ctx, l, 0)
                    }
                } else {
                    self.nav(View::Playlist(i))
                }
            }
            Some((9, ..)) => {
                self.focus_search = true;
                self.nav(View::Search)
            }
            _ => {}
        }
    }

    pub(crate) fn search_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.searched {
            empty_state(
                ui,
                Ic::Search,
                "Search for songs and artists",
                "Only official artist sources are shown.",
            );
            return;
        }
        let cols = grid_cols(ui.available_width());
        let w = (ui.available_width() - 16.0 * (cols as f32 - 1.0)) / cols as f32;
        let mut artist_ev: Option<(usize, bool)> = None;
        let mut tevt = None;
        let list = self.tracks.clone();
        {
            let env = self.env();
            h2(ui, "Artists");
            if self.s_artists {
                skeleton_cards(ui, &env, cols.min(4), w, true);
            } else if self.artists.is_empty() {
                ui.label(
                    egui::RichText::new("No verified artist pages found.")
                        .font(reg(14.0))
                        .color(DIM),
                );
            } else {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    for (i, a) in self.artists.iter().take(cols).enumerate() {
                        let sub = a.followers.map_or("Artist".to_string(), |f| {
                            format!("Artist • {} fans", short_num(f))
                        });
                        match card(
                            ui,
                            &env,
                            ui.scope_id().with(("ac", i)),
                            w,
                            CardKind::Artist(a),
                            &a.name,
                            &sub,
                        ) {
                            CardEvt::Open => artist_ev = Some((i, false)),
                            CardEvt::Play => artist_ev = Some((i, true)),
                            CardEvt::None => {}
                        }
                    }
                });
            }
            h2(ui, "Songs");
            if self.s_tracks {
                skeleton_rows(ui, &env, 7);
            } else if list.is_empty() {
                empty_state(
                    ui,
                    Ic::Note,
                    &format!("No results for “{}”", self.query.trim()),
                    if self.official_only {
                        "Try the artist's name, or turn off “Official only” to include other uploads."
                    } else {
                        "Try the artist's name, or check the spelling."
                    },
                );
            } else {
                tevt = track_table(ui, &env, &list, false);
            }
        }
        if let Some(e) = tevt {
            self.apply(ctx, &list, None, e);
        }
        if let Some((i, play)) = artist_ev {
            let a = self.artists[i].clone();
            self.open_artist(ctx, Some(a.clone()), a.name, None);
            let _ = play; // card play opens the page; tracks start from there
        }
    }

    pub(crate) fn artist_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let list = self.a_tracks.clone();
        let mut tevt = None;
        let mut play_all = false;
        {
            let env = self.env();
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
            let av = Rect::from_min_size(r.min + vec2(0.0, 4.0), vec2(184.0, 184.0));
            match &self.artist {
                Some(a) => {
                    ui.painter().circle_filled(
                        av.center() + vec2(0.0, 6.0),
                        92.0,
                        Color32::from_black_alpha(70),
                    );
                    avatar(ui, a, av);
                    let tx = av.right() + 24.0;
                    let verified = a.followers.is_some() || !a.ids.is_empty();
                    if verified {
                        icon(
                            ui.painter(),
                            Ic::Check,
                            pos2(tx + 8.0, r.top() + 44.0),
                            11.0,
                            Color32::from_rgb(70, 150, 255),
                        );
                        text(
                            ui.painter(),
                            pos2(tx + 24.0, r.top() + 44.0),
                            Align2::LEFT_CENTER,
                            "Official artist",
                            reg(14.0),
                            WHITE,
                        );
                    }
                    text(
                        ui.painter(),
                        pos2(tx, r.top() + 66.0),
                        Align2::LEFT_TOP,
                        &ell(&a.name, r.width() - tx, 64.0),
                        bold(64.0),
                        WHITE,
                    );
                    if let Some(f) = a.followers {
                        text(
                            ui.painter(),
                            pos2(tx, r.top() + 146.0),
                            Align2::LEFT_TOP,
                            &format!("{} fans", short_num(f)),
                            reg(15.0),
                            WHITE,
                        );
                    }
                }
                None => {
                    skeleton_circle(ui.painter(), av.center(), 92.0, env.now, env.reduce);
                    skeleton(
                        ui.painter(),
                        Rect::from_min_size(
                            pos2(av.right() + 24.0, r.top() + 70.0),
                            vec2(320.0, 52.0),
                        ),
                        6.0,
                        env.now,
                        env.reduce,
                    );
                }
            }
            ui.add_space(8.0);
            if !list.is_empty() {
                let (br, resp) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::click());
                let hh = ease(ctx, ui.scope_id().with("aplay"), resp.hovered(), env.reduce);
                let sc = if resp.is_pointer_button_down_on() {
                    0.94
                } else {
                    1.0 + 0.06 * hh
                };
                ui.painter().circle_filled(br.center(), 28.0 * sc, GREEN);
                play_icon(
                    ui.painter(),
                    br.center() + vec2(1.0, 0.0),
                    11.0,
                    Color32::BLACK,
                );
                play_all = resp.clicked();
            }
            h2(ui, "Popular");
            if self.a_loading {
                skeleton_rows(ui, &env, 8);
            } else if list.is_empty() {
                empty_state(
                    ui,
                    Ic::Note,
                    "No official tracks found",
                    "This artist has no playable official uploads right now.",
                );
            } else {
                tevt = track_table(ui, &env, &list, false);
            }
        }
        if play_all {
            self.play(ctx, list.clone(), 0);
        }
        if let Some(e) = tevt {
            self.apply(ctx, &list, None, e);
        }
    }

    pub(crate) fn collection(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        pl: Option<usize>,
        list: &[Track],
    ) {
        let reduce = self.reduce;
        let mut play_all = false;
        let mut delete = false;
        let mut rename: Option<String> = None;
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
        let cv = Rect::from_min_size(r.min + vec2(0.0, 4.0), vec2(184.0, 184.0));
        ui.painter().rect_filled(
            cv.translate(vec2(0.0, 6.0)),
            6.0,
            Color32::from_black_alpha(70),
        );
        match pl.and_then(|i| self.playlists.get(i)) {
            Some(p) => playlist_cover(ui, p, cv, 6.0),
            None => {
                ui.painter()
                    .rect_filled(cv, 6.0, Color32::from_rgb(80, 56, 220));
                icon(ui.painter(), Ic::HeartFill, cv.center(), 72.0, WHITE);
            }
        }
        let tx = cv.right() + 24.0;
        let mins: u64 = list.iter().map(|t| t.secs).sum::<u64>() / 60;
        let sub = format!(
            "{} songs{}",
            list.len(),
            if mins > 0 {
                format!(" • about {} min", mins)
            } else {
                String::new()
            }
        );
        match pl {
            None => {
                text(
                    ui.painter(),
                    pos2(tx, r.top() + 70.0),
                    Align2::LEFT_TOP,
                    "Liked Songs",
                    bold(56.0),
                    WHITE,
                );
            }
            Some(i) => {
                // click-to-rename: the title is a borderless text field
                let rect = Rect::from_min_size(
                    pos2(tx, r.top() + 58.0),
                    vec2((r.right() - tx).max(120.0), 76.0),
                );
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                let mut name = self.playlists[i].name.clone();
                let resp = child.add(
                    egui::TextEdit::singleline(&mut name)
                        .frame(egui::Frame::NONE)
                        .font(bold(56.0))
                        .desired_width(rect.width()),
                );
                if resp.changed() {
                    rename = Some(name);
                }
            }
        }
        text(
            ui.painter(),
            pos2(tx, r.top() + 148.0),
            Align2::LEFT_TOP,
            &sub,
            reg(15.0),
            WHITE,
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if !list.is_empty() {
                let (br, resp) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::click());
                let hh = ease(ctx, ui.scope_id().with("cplay"), resp.hovered(), reduce);
                let sc = if resp.is_pointer_button_down_on() {
                    0.94
                } else {
                    1.0 + 0.06 * hh
                };
                ui.painter().circle_filled(br.center(), 28.0 * sc, GREEN);
                play_icon(
                    ui.painter(),
                    br.center() + vec2(1.0, 0.0),
                    11.0,
                    Color32::BLACK,
                );
                play_all = resp.clicked();
            }
            if pl.is_some() {
                let (dr, _) = ui.allocate_exact_size(vec2(40.0, 56.0), Sense::hover());
                let armed = self.del_confirm.is_some();
                let rr = Rect::from_center_size(dr.center(), vec2(36.0, 36.0));
                if icon_btn(
                    ui,
                    ui.scope_id().with("del"),
                    rr,
                    Ic::Trash,
                    20.0,
                    if armed {
                        Color32::from_rgb(240, 90, 90)
                    } else {
                        DIM
                    },
                    WHITE,
                    reduce,
                )
                .on_hover_text("Delete playlist")
                .clicked()
                {
                    if armed {
                        delete = true;
                    } else {
                        self.del_confirm = pl.map(|i| (i, self.now));
                        self.toast("Click the trash icon again to delete this playlist");
                    }
                }
            }
        });
        ui.add_space(8.0);
        if list.is_empty() {
            empty_state(
                ui,
                Ic::Note,
                if pl.is_some() {
                    "This playlist is empty"
                } else {
                    "Songs you like will appear here"
                },
                if pl.is_some() {
                    "Use the + button on any song to add it here."
                } else {
                    "Press the heart on a song to save it."
                },
            );
        } else {
            let evt = {
                let env = self.env();
                track_table(ui, &env, list, pl.is_some())
            };
            if let Some(e) = evt {
                self.apply(ctx, list, pl, e);
            }
        }
        if play_all {
            self.play(ctx, list.to_vec(), 0);
        }
        if let (Some(i), Some(n)) = (pl, rename) {
            self.playlists[i].name = n;
            db::save_playlists(&self.playlists);
        }
        if delete {
            if let Some(i) = pl {
                self.playlists.remove(i);
                db::save_playlists(&self.playlists);
                self.del_confirm = None;
                self.back.retain(|v| !matches!(v, View::Playlist(_)));
                self.view = View::Home;
                self.nav_t = self.now;
                self.toast("Playlist deleted");
            }
        }
    }

    // ── floating layers: add-to-playlist menu, toast ──
    /// Slim status line while helper programs download (first run) or when setup failed.
    pub(crate) fn setup_banner(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use crate::setup::{self, Step};
        let status = setup::status();
        if !status.needs_attention() {
            return;
        }
        let failed = [&status.yt_dlp, &status.js]
            .into_iter()
            .find_map(|s| match s {
                Step::Failed(e) => Some(e.clone()),
                _ => None,
            });
        let busy = [&status.yt_dlp, &status.js]
            .into_iter()
            .find_map(|s| match s {
                Step::Running(m) => Some(m.clone()),
                _ => None,
            });
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
        ui.painter().rect_filled(r, 18.0, ELEV);
        let c = pos2(r.left() + 22.0, r.center().y);
        match (&failed, &busy) {
            (Some(e), _) => {
                icon(
                    ui.painter(),
                    Ic::Close,
                    c,
                    12.0,
                    Color32::from_rgb(240, 90, 90),
                );
                text(
                    ui.painter(),
                    pos2(r.left() + 40.0, r.center().y),
                    Align2::LEFT_CENTER,
                    &ell(&format!("Setup failed: {e}"), r.width() - 160.0, 13.0),
                    reg(13.0),
                    WHITE,
                );
                let b =
                    Rect::from_center_size(pos2(r.right() - 48.0, r.center().y), vec2(72.0, 26.0));
                let resp = ui.interact(b, ui.make_persistent_id("setup-retry"), Sense::click());
                ui.painter().rect_filled(
                    b,
                    13.0,
                    if resp.hovered() {
                        WHITE
                    } else {
                        Color32::from_gray(225)
                    },
                );
                text(
                    ui.painter(),
                    b.center(),
                    Align2::CENTER_CENTER,
                    "Retry",
                    bold(13.0),
                    Color32::BLACK,
                );
                if resp.clicked() {
                    setup::start(Some(ctx.clone()));
                }
            }
            (None, Some(m)) => {
                spinner(ui.painter(), c, 7.0, GREEN, self.now, self.reduce);
                text(
                    ui.painter(),
                    pos2(r.left() + 40.0, r.center().y),
                    Align2::LEFT_CENTER,
                    &format!("Setting up the audio engine (first run only): {m}"),
                    reg(13.0),
                    WHITE,
                );
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            _ => {}
        }
        ui.add_space(8.0);
    }

    pub(crate) fn overlays(&mut self, ctx: &egui::Context) {
        if let Some((t, pos, born)) = self.adding.clone() {
            let mut close = false;
            let mut chosen: Option<Option<usize>> = None;
            let shadow = egui::epaint::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(150),
            };
            let area = egui::Area::new(egui::Id::unique("add-menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(40, 40, 40))
                        .corner_radius(8.0)
                        .inner_margin(8.0)
                        .shadow(shadow)
                        .show(ui, |ui| {
                            ui.set_width(240.0);
                            ui.label(
                                egui::RichText::new("Add to playlist")
                                    .font(reg(12.0))
                                    .color(MUTED),
                            );
                            ui.add_space(4.0);
                            let item = |ui: &mut egui::Ui, id: &str, ic: Ic, label: &str| -> bool {
                                let (r, resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 36.0),
                                    Sense::click(),
                                );
                                let hh = ease(
                                    ctx,
                                    egui::Id::unique(("am", id)),
                                    resp.hovered(),
                                    self.reduce,
                                );
                                ui.painter().rect_filled(
                                    r,
                                    4.0,
                                    Color32::from_white_alpha((20.0 * hh) as u8),
                                );
                                icon(
                                    ui.painter(),
                                    ic,
                                    pos2(r.left() + 18.0, r.center().y),
                                    16.0,
                                    WHITE,
                                );
                                text(
                                    ui.painter(),
                                    pos2(r.left() + 40.0, r.center().y),
                                    Align2::LEFT_CENTER,
                                    &ell(label, r.width() - 52.0, 14.0),
                                    reg(14.0),
                                    WHITE,
                                );
                                if resp.hovered() {
                                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                resp.clicked()
                            };
                            if item(ui, "new", Ic::Plus, "New playlist") {
                                chosen = Some(None);
                            }
                            for (i, p) in self.playlists.iter().enumerate() {
                                if item(ui, &format!("p{i}"), Ic::Note, &p.name) {
                                    chosen = Some(Some(i));
                                }
                            }
                        });
                });
            let outside = ctx.input(|i| i.pointer.any_click())
                && !area.response.contains_pointer()
                && self.now - born > 0.1;
            if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            if let Some(c) = chosen {
                match c {
                    None => {
                        let i = self.new_playlist(Some(t.clone()));
                        self.toast(format!("Added to {}", self.playlists[i].name));
                    }
                    Some(i) => {
                        if self.playlists[i].tracks.contains(&t) {
                            self.toast(format!("Already in {}", self.playlists[i].name));
                        } else {
                            self.playlists[i].tracks.push(t.clone());
                            db::save_playlists(&self.playlists);
                            self.toast(format!("Added to {}", self.playlists[i].name));
                        }
                    }
                }
                close = true;
            }
            if close {
                self.adding = None;
            }
        }
        if let Some((m, born)) = self.toast.clone() {
            let a = if self.reduce {
                1.0
            } else {
                egui::emath::easing::cubic_out(((self.now - born) / 0.2).clamp(0.0, 1.0) as f32)
            };
            egui::Area::new(egui::Id::unique("toast"))
                .order(egui::Order::Tooltip)
                .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -108.0 + (1.0 - a) * 12.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.set_opacity(a);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(50, 50, 50))
                        .corner_radius(20.0)
                        .inner_margin(egui::Margin::symmetric(18, 10))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 6],
                            blur: 20,
                            spread: 0,
                            color: Color32::from_black_alpha(140),
                        })
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(m).font(reg(14.0)).color(WHITE));
                        });
                });
        }
    }
}
