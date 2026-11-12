mod catalog;
mod db;
mod model;
mod setup;
mod stream;
mod ui;
mod util;
mod ytdlp;

use crate::catalog::{find_artists, find_tracks};
use crate::stream::{match_official, play_catalogue};
use crate::ui::App;
use eframe::egui::{self};

fn main() -> eframe::Result {
    // `spotty --probe "query"`: print how the first result would be sourced (no window, no audio device)
    if let Some(q) = std::env::args().skip_while(|a| a != "--probe").nth(1) {
        setup::run_blocking();
        let tr = find_tracks(&q).unwrap_or_default();
        let ar = find_artists(&q, &tr).unwrap_or_default();
        println!(
            "artists: {:?}",
            ar.iter()
                .map(|a| (&a.name, a.followers))
                .collect::<Vec<_>>()
        );
        println!("tracks: {}", tr.len());
        for t in tr.iter().take(3) {
            let yt = match_official("ytsearch", t);
            let sc = match_official("scsearch", t);
            println!(
                "{} - {} ({}s) src={} cover={}",
                t.artist,
                t.title,
                t.secs,
                t.src,
                !t.thumb.is_empty()
            );
            println!("   youtube official: {:?}", yt);
            println!("   soundcloud official: {:?}", sc);
            println!("   preview: {}", !t.preview.is_empty());
        }
        if let Some(t) = tr.first() {
            match play_catalogue(t) {
                Ok((_, pv, note)) => {
                    println!("play_catalogue: ok, preview_only={pv}, note={note:?}")
                }
                Err(e) => println!("play_catalogue: FAILED {e}"),
            }
        }
        return Ok(());
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 680.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Spotty")
            .with_app_id("spotty"),
        ..Default::default()
    };
    let opts = match window_icon() {
        Some(icon) => eframe::NativeOptions {
            viewport: opts.viewport.with_icon(icon),
            ..opts
        },
        None => opts,
    };
    ytdlp::start_pot_provider();
    let result = eframe::run_native("Spotty", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))));
    ytdlp::stop_pot_provider();
    result
}

fn window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .ok()?
        .to_rgba8();
    Some(egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}
