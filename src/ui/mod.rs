mod anim;
mod icons;
mod screens;
mod theme;
mod widgets;

use crate::catalog::{artist_by_id, artist_tracks, find_artists, find_tracks};
use crate::db;
use crate::model::{Artist, Msg, Playlist, Track, View};
use crate::stream::{download, stream};
use crate::ui::theme::{BASE, GREEN, WHITE, setup_fonts};
use crate::ui::widgets::{Env, Evt};
use crate::ytdlp::plain_err;
use eframe::egui::{self, Color32, Pos2, Stroke, vec2};
use rodio::{DeviceSinkBuilder, Player};
use std::collections::HashSet;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

pub(crate) struct App {
    view: View,
    back: Vec<View>,
    nav_t: f64,
    now: f64,
    reduce: bool,
    greeting: String,

    query: String,
    qgen: u64,
    tracks: Vec<Track>,
    artists: Vec<Artist>,
    s_tracks: bool,
    s_artists: bool,
    searched: bool,
    focus_search: bool,

    artist: Option<Artist>,
    a_tracks: Vec<Track>,
    a_loading: bool,
    agen: u64,

    liked: Vec<Track>,
    playlists: Vec<Playlist>,
    recent: Vec<Track>,

    queue: Vec<Track>,
    cur: Option<usize>,
    loading: bool,
    started: bool,
    unavailable: HashSet<String>,

    toast: Option<(String, f64)>,
    adding: Option<(Track, Pos2, f64)>,
    del_confirm: Option<(usize, f64)>,

    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,
    _sink: rodio::MixerDeviceSink,
    player: Player,
    req: usize,
    seek: f32,
    dragging: bool,
    muted_vol: Option<f32>,
    preview: bool,
}

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        setup_fonts(&cc.egui_ctx);
        let mut st = (*cc.egui_ctx.global_style()).clone();
        st.visuals = egui::Visuals::dark();
        st.visuals.panel_fill = BASE;
        st.visuals.override_text_color = Some(WHITE);
        st.visuals.selection.bg_fill = GREEN.gamma_multiply(0.5);
        st.visuals.selection.stroke = Stroke::new(1.0, WHITE);
        st.visuals.text_cursor.stroke = Stroke::new(2.0, GREEN);
        st.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        st.spacing.item_spacing = vec2(8.0, 6.0);
        st.spacing.scroll = egui::style::ScrollStyle::floating();
        cc.egui_ctx.set_global_style(st);

        let sink = DeviceSinkBuilder::open_default_sink().unwrap_or_else(|e| {
            eprintln!("Spotty needs an audio output device but could not open one: {e}");
            std::process::exit(1);
        });
        crate::setup::start(Some(cc.egui_ctx.clone()));
        let player = Player::connect_new(sink.mixer());
        let (tx, rx) = mpsc::channel();
        let hour: u32 = Command::new("date")
            .arg("+%H")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(12);
        let greeting = match hour {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            _ => "Good evening",
        }
        .to_string();
        Self {
            view: View::Home,
            back: vec![],
            nav_t: -10.0,
            now: 0.0,
            reduce: std::env::var("SPOTTY_REDUCE_MOTION").is_ok_and(|v| v != "0"),
            greeting,
            query: String::new(),
            qgen: 0,
            tracks: vec![],
            artists: vec![],
            s_tracks: false,
            s_artists: false,
            searched: false,
            focus_search: false,
            artist: None,
            a_tracks: vec![],
            a_loading: false,
            agen: 0,
            liked: db::liked(),
            playlists: db::playlists(),
            recent: db::recent(),
            queue: vec![],
            cur: None,
            loading: false,
            started: false,
            unavailable: HashSet::new(),
            toast: None,
            adding: None,
            del_confirm: None,
            tx,
            rx,
            _sink: sink,
            player,
            req: 0,
            seek: 0.0,
            dragging: false,
            muted_vol: None,
            preview: false,
        }
    }

    // ── navigation & actions ──
    pub(crate) fn nav(&mut self, v: View) {
        if v != self.view {
            self.back.push(std::mem::replace(&mut self.view, v));
            self.nav_t = self.now;
        }
    }
    pub(crate) fn go_back(&mut self) {
        if let Some(v) = self.back.pop() {
            self.view = v;
            self.nav_t = self.now;
        }
    }
    pub(crate) fn toast(&mut self, m: impl Into<String>) {
        self.toast = Some((m.into(), self.now));
    }

    pub(crate) fn search(&mut self, ctx: &egui::Context) {
        let q = self.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.qgen += 1;
        let g = self.qgen;
        self.searched = true;
        self.s_tracks = true;
        self.s_artists = true;
        self.tracks.clear();
        self.artists.clear();
        self.nav(View::Search);
        let (tx, c) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let tr = find_tracks(&q);
            let hint = tr.clone().unwrap_or_default();
            let _ = tx.send(Msg::Tracks(g, tr));
            c.request_repaint();
            let _ = tx.send(Msg::Artists(g, find_artists(&q, &hint)));
            c.request_repaint();
        });
    }

    pub(crate) fn open_artist(
        &mut self,
        ctx: &egui::Context,
        known: Option<Artist>,
        name: String,
        id: Option<String>,
    ) {
        self.agen += 1;
        let g = self.agen;
        self.artist = known.clone();
        self.a_tracks.clear();
        self.a_loading = true;
        self.nav(View::Artist);
        let (tx, c) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = (|| {
                let a = match (known, id) {
                    (Some(a), _) => a,
                    (None, Some(id)) if !id.is_empty() => artist_by_id(&id)?,
                    _ => find_artists(&name, &[])?
                        .into_iter()
                        .next()
                        .ok_or("No official artist page found")?,
                };
                let t = artist_tracks(&a)?;
                Ok((a, t))
            })();
            let _ = tx.send(Msg::ArtistPage(g, r));
            c.request_repaint();
        });
    }

    pub(crate) fn play(&mut self, ctx: &egui::Context, queue: Vec<Track>, i: usize) {
        self.queue = queue;
        self.go(ctx, i);
    }
    pub(crate) fn go(&mut self, ctx: &egui::Context, i: usize) {
        let Some(t) = self.queue.get(i).cloned() else {
            return;
        };
        self.req += 1;
        self.player.clear();
        self.cur = Some(i);
        self.loading = true;
        self.started = false;
        self.seek = 0.0;
        self.recent.retain(|r| r != &t);
        self.recent.insert(0, t.clone());
        self.recent.truncate(12);
        db::save_recent(&self.recent);
        stream(self.req, t, self.tx.clone(), ctx.clone());
    }
    pub(crate) fn toggle_like(&mut self, t: &Track) {
        match self.liked.iter().position(|x| x == t) {
            Some(p) => {
                self.liked.remove(p);
            }
            None => self.liked.insert(0, t.clone()),
        }
        db::save_liked(&self.liked);
    }
    pub(crate) fn new_playlist(&mut self, first: Option<Track>) -> usize {
        let mut n = self.playlists.len() + 1;
        while self
            .playlists
            .iter()
            .any(|p| p.name == format!("My playlist #{n}"))
        {
            n += 1;
        }
        self.playlists.push(Playlist {
            name: format!("My playlist #{n}"),
            tracks: first.into_iter().collect(),
        });
        db::save_playlists(&self.playlists);
        self.playlists.len() - 1
    }
    pub(crate) fn env(&self) -> Env<'_> {
        Env {
            liked: &self.liked,
            unavailable: &self.unavailable,
            playing: self.cur.and_then(|c| self.queue.get(c)),
            live: self.started && !self.player.is_paused() && !self.player.empty(),
            loading: self.loading,
            now: self.now,
            reduce: self.reduce,
        }
    }

    pub(crate) fn apply(&mut self, ctx: &egui::Context, list: &[Track], pl: Option<usize>, e: Evt) {
        match e {
            Evt::Play(i) => self.play(ctx, list.to_vec(), i),
            Evt::Like(i) => {
                let t = list[i].clone();
                self.toggle_like(&t)
            }
            Evt::Add(i, p) => self.adding = Some((list[i].clone(), p, self.now)),
            Evt::Dl(i) => {
                self.toast(format!("Downloading “{}”…", list[i].title));
                download(list[i].clone(), self.tx.clone(), ctx.clone());
            }
            Evt::Remove(i) => {
                if let Some(p) = pl {
                    self.playlists[p].tracks.remove(i);
                    db::save_playlists(&self.playlists);
                }
            }
            Evt::Artist(i) => {
                let (n, id) = (list[i].artist.clone(), list[i].artist_id.clone());
                self.open_artist(ctx, None, n, Some(id))
            }
        }
    }

    pub(crate) fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Tracks(g, r) if g == self.qgen => {
                    self.s_tracks = false;
                    match r {
                        Ok(t) => self.tracks = t,
                        Err(e) => self.toast(plain_err(&e)),
                    }
                }
                Msg::Artists(g, r) if g == self.qgen => {
                    self.s_artists = false;
                    if let Ok(a) = r {
                        self.artists = a;
                    }
                }
                Msg::ArtistPage(g, r) if g == self.agen => {
                    self.a_loading = false;
                    match r {
                        Ok((a, t)) => {
                            self.artist = Some(a);
                            self.a_tracks = t;
                        }
                        Err(e) => self.toast(plain_err(&e)),
                    }
                }
                Msg::Ready(g, _t, dec, pv) if g == self.req => {
                    self.loading = false;
                    self.preview = pv;
                    self.started = true;
                    self.player.append(dec);
                    self.player.play();
                }
                Msg::Fail(g, id, e) if g == self.req => {
                    self.loading = false;
                    self.unavailable.insert(id);
                    self.toast(plain_err(&e));
                }
                Msg::Toast(m) => self.toast(m),
                _ => {}
            }
        }
        // advance only after a track really played to its end
        if self.started && !self.loading && self.player.empty() {
            match self.cur.filter(|c| c + 1 < self.queue.len()) {
                Some(c) => self.go(ctx, c + 1),
                None => self.started = false,
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        let narrow = ui.available_width() < 920.0;
        let animating = self.loading
            || self.s_tracks
            || self.s_artists
            || self.a_loading
            || self.now - self.nav_t < 0.3
            || (self.started && !self.player.is_paused());
        ctx.request_repaint_after(Duration::from_millis(if animating && !self.reduce {
            16
        } else {
            250
        }));

        self.drain(&ctx);
        if self.toast.as_ref().is_some_and(|(_, t)| self.now - t > 4.5) {
            self.toast = None;
        }
        if self.del_confirm.is_some_and(|(_, t)| self.now - t > 3.0) {
            self.del_confirm = None;
        }

        // space = play/pause
        if !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Space))
            && self.started
        {
            if self.player.is_paused() {
                self.player.play()
            } else {
                self.player.pause()
            }
        }

        self.player_bar(ui, &ctx);
        self.sidebar(ui, &ctx, narrow);
        self.main_panel(ui, &ctx);
        self.overlays(&ctx);
    }
}
