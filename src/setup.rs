//! First-run setup: downloads, verifies and keeps up to date the helper programs Spotty drives
//! (yt-dlp, plus a JavaScript runtime for it). Runs in a background thread; the UI shows progress.
//!
//! Everything lands in the per-user cache directory. Downloads are checksum-verified against the
//! checksum files the projects publish next to their releases, written to `*.part` and renamed
//! into place, so an interrupted download never leaves a half-written executable behind.

use crate::db::cache_dir;
use eframe::egui;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Condvar, LazyLock, Mutex},
    time::{Duration, SystemTime},
};

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Step {
    Pending,
    Running(String),
    Ready,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Status {
    pub(crate) yt_dlp: Step,
    pub(crate) js: Step,
}

impl Status {
    /// Setup is still downloading, or something failed: the UI should say so.
    pub(crate) fn needs_attention(&self) -> bool {
        [&self.yt_dlp, &self.js]
            .iter()
            .any(|s| matches!(s, Step::Running(_) | Step::Failed(_)))
    }

    fn settled(&self) -> bool {
        [&self.yt_dlp, &self.js]
            .iter()
            .all(|s| matches!(s, Step::Ready | Step::Failed(_)))
    }
}

struct State {
    status: Status,
    yt_dlp: Option<PathBuf>,
    js: Option<String>, // value for `--js-runtimes`
    running: bool,
}

static STATE: LazyLock<(Mutex<State>, Condvar)> = LazyLock::new(|| {
    (
        Mutex::new(State {
            status: Status {
                yt_dlp: Step::Pending,
                js: Step::Pending,
            },
            yt_dlp: None,
            js: None,
            running: false,
        }),
        Condvar::new(),
    )
});

fn lock() -> std::sync::MutexGuard<'static, State> {
    STATE.0.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn status() -> Status {
    lock().status.clone()
}

pub(crate) fn js_runtime() -> Option<String> {
    lock().js.clone()
}

/// Block until setup has settled (everything ready or failed), then return the yt-dlp path.
pub(crate) fn wait_ready(timeout: Duration) -> Result<PathBuf, String> {
    let (m, cv) = &*STATE;
    let g = m.lock().unwrap_or_else(|e| e.into_inner());
    let (g, _) = cv
        .wait_timeout_while(g, timeout, |s| !s.status.settled())
        .unwrap_or_else(|e| e.into_inner());
    match (&g.yt_dlp, &g.status.yt_dlp) {
        (Some(p), _) => Ok(p.clone()),
        (None, Step::Failed(e)) => Err(format!("yt-dlp is not installed: {e}")),
        _ => Err("Still setting up, try again in a moment".into()),
    }
}

fn set(f: impl FnOnce(&mut State), ctx: &Option<egui::Context>) {
    f(&mut lock());
    STATE.1.notify_all();
    if let Some(c) = ctx {
        c.request_repaint();
    }
}

/// Start setup in the background (no-op if it is already running).
pub(crate) fn start(ctx: Option<egui::Context>) {
    {
        let mut s = lock();
        if s.running {
            return;
        }
        s.running = true;
        s.status = Status {
            yt_dlp: Step::Pending,
            js: Step::Pending,
        };
        s.yt_dlp = None;
        s.js = None;
    }
    std::thread::spawn(move || {
        run(&ctx);
        set(|s| s.running = false, &ctx);
    });
}

/// Run setup on the calling thread (used by `--probe`).
pub(crate) fn run_blocking() {
    start(None);
    let _ = wait_ready(Duration::from_secs(600));
}

fn run(ctx: &Option<egui::Context>) {
    let progress = |part: &'static str| {
        let ctx = ctx.clone();
        move |msg: String| {
            set(
                |s| match part {
                    "yt" => s.status.yt_dlp = Step::Running(msg),
                    _ => s.status.js = Step::Running(msg),
                },
                &ctx,
            )
        }
    };
    match ensure_yt_dlp(&progress("yt")) {
        Ok(bin) => {
            set(
                |s| {
                    s.yt_dlp = Some(bin.clone());
                    s.status.yt_dlp = Step::Ready;
                },
                ctx,
            );
            update_yt_dlp_if_stale(&bin); // after "ready": playback never waits for an update check
        }
        Err(e) => set(|s| s.status.yt_dlp = Step::Failed(e), ctx),
    }
    match ensure_js_runtime(&progress("js")) {
        Ok(rt) => set(
            |s| {
                s.js = Some(rt);
                s.status.js = Step::Ready;
            },
            ctx,
        ),
        Err(e) => set(|s| s.status.js = Step::Failed(e), ctx),
    }
}

// ───────────────────────────── components ─────────────────────────────

/// (yt-dlp release asset, deno release archive) for this machine.
fn platform() -> Result<(&'static str, &'static str), String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok(("yt-dlp_linux", "deno-x86_64-unknown-linux-gnu.zip")),
        ("linux", "aarch64") => Ok(("yt-dlp_linux_aarch64", "deno-aarch64-unknown-linux-gnu.zip")),
        (os, arch) => Err(format!("unsupported platform {os}/{arch}")),
    }
}

fn bin_dir() -> PathBuf {
    let d = cache_dir().join("bin");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn ensure_yt_dlp(report: &impl Fn(String)) -> Result<PathBuf, String> {
    let (asset, _) = platform()?;
    let bin = bin_dir().join("yt-dlp");
    if bin.exists() {
        return Ok(bin);
    }
    report("Downloading yt-dlp".into());
    let base = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
    let sums = fetch_text(&format!("{base}/SHA2-256SUMS"))?;
    let want = find_sum(&sums, asset).ok_or("no published checksum for this platform")?;
    download(
        &format!("{base}/{asset}"),
        &bin,
        &want,
        "Downloading yt-dlp",
        report,
    )?;
    make_executable(&bin)?;
    touch(&bin.with_extension("checked"));
    Ok(bin)
}

/// yt-dlp breaks when YouTube changes, so refresh it about once a day with its own self-updater.
fn update_yt_dlp_if_stale(bin: &Path) {
    let stamp = bin.with_extension("checked");
    let fresh = std::fs::metadata(&stamp)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < Duration::from_secs(24 * 3600));
    if !fresh
        && Command::new(bin)
            .arg("-U")
            .output()
            .is_ok_and(|o| o.status.success())
    {
        touch(&stamp);
    }
}

/// yt-dlp needs a JavaScript runtime for YouTube's challenges. Use one already installed, else fetch deno.
fn ensure_js_runtime(report: &impl Fn(String)) -> Result<String, String> {
    for rt in ["deno", "node", "bun"] {
        if Command::new(rt)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Ok(rt.to_string());
        }
    }
    let (_, archive) = platform()?;
    let deno = bin_dir().join("deno");
    if !deno.exists() {
        let base = "https://github.com/denoland/deno/releases/latest/download";
        let want = fetch_text(&format!("{base}/{archive}.sha256sum"))?
            .split_whitespace()
            .next()
            .ok_or("empty checksum file")?
            .to_lowercase();
        let zip_path = bin_dir().join("deno.zip");
        download(
            &format!("{base}/{archive}"),
            &zip_path,
            &want,
            "Downloading JavaScript runtime",
            report,
        )?;
        let result = extract_one(&zip_path, "deno", &deno);
        let _ = std::fs::remove_file(&zip_path);
        result?;
        make_executable(&deno)?;
    }
    Ok(format!("deno:{}", deno.display()))
}

// ───────────────────────────── helpers ─────────────────────────────

fn fetch_text(url: &str) -> Result<String, String> {
    ureq::get(url)
        .call()
        .map_err(|e| format!("cannot reach {url}: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Checksum for `name` in a `sha256sum`-style listing ("<hex>  <name>" per line).
fn find_sum(listing: &str, name: &str) -> Option<String> {
    listing.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let (hash, file) = (parts.next()?, parts.next()?);
        (file.trim_start_matches('*') == name).then(|| hash.to_lowercase())
    })
}

/// Stream `url` to `dest`, verify its SHA-256, and only then move it into place.
fn download(
    url: &str,
    dest: &Path,
    want: &str,
    label: &str,
    report: &impl Fn(String),
) -> Result<(), String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let total = resp.body().content_length();
    let mut body = resp.into_body();
    let mut reader = body.with_config().limit(u64::MAX).reader();
    let part = dest.with_extension("part");
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let (mut hasher, mut buf, mut done, mut last) =
        (Sha256::new(), vec![0u8; 64 * 1024], 0u64, 0u64);
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if let Some(t) = total.filter(|t| *t > 0) {
            let pct = done * 100 / t;
            if pct != last {
                last = pct;
                report(format!("{label} {pct}%"));
            }
        }
    }
    drop(file);
    let got = format!("{:x}", hasher.finalize());
    if got != want {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "checksum mismatch for {label} (expected {want}, got {got})"
        ));
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())
}

fn extract_one(zip_path: &Path, entry: &str, out: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut f = zip.by_name(entry).map_err(|e| e.to_string())?;
    let part = out.with_extension("part");
    std::io::copy(
        &mut f,
        &mut std::fs::File::create(&part).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&part, out).map_err(|e| e.to_string())
}

fn make_executable(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

fn touch(p: &Path) {
    let _ = std::fs::write(p, b"");
}

/// Hex SHA-256 of a byte slice (test helper, also documents the format `download` compares against).
#[cfg(test)]
fn sha256_hex(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_listing_and_hash() {
        let listing = "aaa111  yt-dlp\nBBB222 *yt-dlp_linux\nccc333  yt-dlp_macos\n";
        assert_eq!(find_sum(listing, "yt-dlp_linux").as_deref(), Some("bbb222"));
        assert_eq!(find_sum(listing, "yt-dlp").as_deref(), Some("aaa111"));
        assert_eq!(find_sum(listing, "missing"), None);
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn status_flags() {
        let ready = Status {
            yt_dlp: Step::Ready,
            js: Step::Ready,
        };
        assert!(ready.settled() && !ready.needs_attention());
        let busy = Status {
            yt_dlp: Step::Ready,
            js: Step::Running("x".into()),
        };
        assert!(!busy.settled() && busy.needs_attention());
        let failed = Status {
            yt_dlp: Step::Failed("e".into()),
            js: Step::Ready,
        };
        assert!(failed.settled() && failed.needs_attention());
    }
}
