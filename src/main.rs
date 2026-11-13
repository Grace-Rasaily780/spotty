#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod catalog;
mod db;
mod model;
mod pot;
mod rank;
mod setup;
mod stream;
mod ui;
mod util;
mod ytdlp;

use crate::catalog::{find_artists, find_tracks};
use crate::stream::resolve;
use crate::ui::App;
use eframe::egui::{self};

fn main() -> eframe::Result {
    // `spotty --probe "query"`: print how the first result would be sourced (no window, no audio device)
    if let Some(q) = std::env::args().skip_while(|a| a != "--probe").nth(1) {
        setup::run_blocking();
        let official_only = !std::env::args().any(|a| a == "--all");
        let tr = find_tracks(&q, official_only).unwrap_or_default();
        let ar = find_artists(&q).unwrap_or_default();
        println!(
            "artists: {:?}",
            ar.iter()
                .map(|a| (&a.name, a.followers))
                .collect::<Vec<_>>()
        );
        println!("tracks (official_only={official_only}): {}", tr.len());
        for t in tr.iter().take(10) {
            println!(
                "  {} - {} ({}s){}",
                t.artist,
                t.title,
                t.secs,
                if t.unofficial { "  [unofficial]" } else { "" }
            );
        }
        if let Some(t) = tr.first() {
            match resolve(t) {
                Ok(_) => println!("first result resolves to a stream: ok"),
                Err(e) => println!("first result resolve FAILED: {e}"),
            }
        }
        pot::stop();
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
    let result = eframe::run_native("Spotty", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))));
    pot::stop();
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
