use crate::db::cache_dir;
use crate::model::Track;
use crate::util::{key, norm, quiet};
use std::process::Command;
use std::time::Duration;

/// A ready-to-run yt-dlp command. Waits (first run only) for setup to finish downloading it.
pub(crate) fn command() -> Result<Command, String> {
    let bin = crate::setup::wait_ready(Duration::from_secs(600))?;
    let mut c = Command::new(bin);
    quiet(&mut c);
    if let Some(rt) = crate::setup::js_runtime() {
        c.args(["--js-runtimes", &rt]);
    }
    if let Ok(browser) = std::env::var("SPOTTY_COOKIES") {
        c.args(["--cookies-from-browser", &browser]);
    }
    Ok(c)
}

pub(crate) fn ytdlp_err(o: &std::process::Output) -> String {
    let e = String::from_utf8_lossy(&o.stderr);
    e.lines()
        .rev()
        .find(|l| l.contains("ERROR"))
        .unwrap_or("yt-dlp failed")
        .trim()
        .chars()
        .take(220)
        .collect()
}

pub(crate) fn run_json(args: &[&str]) -> Result<serde_json::Value, String> {
    let o = command()?
        .args(args)
        .output()
        .map_err(|e| format!("cannot run yt-dlp: {e}"))?;
    if !o.status.success() {
        return Err(ytdlp_err(&o));
    }
    serde_json::from_slice(&o.stdout).map_err(|e| e.to_string())
}

pub(crate) static YT_BLOCKED: std::sync::Mutex<Option<std::time::Instant>> =
    std::sync::Mutex::new(None);

pub(crate) const RETRY_YT: Duration = Duration::from_secs(300);

/// YouTube counts as blocked for 5 min after a bot-check; then we probe it again automatically.
pub(crate) fn yt_blocked() -> bool {
    YT_BLOCKED
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < RETRY_YT)
}

pub(crate) fn set_yt_blocked(b: bool) {
    *YT_BLOCKED.lock().unwrap() = b.then(std::time::Instant::now);
}

pub(crate) fn is_block_err(e: &str) -> bool {
    ["Sign in to confirm", "429", "not a bot", "HTTP Error 403"]
        .iter()
        .any(|m| e.contains(m))
}

pub(crate) fn plain_err(e: &str) -> String {
    if is_block_err(e) {
        "YouTube is limiting this connection right now.".into()
    } else {
        e.to_string()
    }
}

pub(crate) struct Raw {
    pub(crate) t: Track,
    pub(crate) chan: String,
    pub(crate) chan_id: String,
    pub(crate) verified: bool,
}

pub(crate) fn raw_search(prefix: &str, q: &str, n: usize) -> Result<Vec<Raw>, String> {
    let v = run_json(&["--flat-playlist", "-J", &format!("{prefix}{n}:{q}")])?;
    let sc = prefix == "scsearch";
    Ok(v["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let chan = e["channel"]
                .as_str()
                .or(e["uploader"].as_str())
                .unwrap_or("")
                .to_string();
            Some(Raw {
                t: Track {
                    id: e["id"]
                        .as_str()
                        .map(str::to_string)
                        .or(e["id"].as_i64().map(|i| i.to_string()))?,
                    title: e["title"].as_str()?.into(),
                    artist: chan.trim_end_matches(" - Topic").into(),
                    secs: e["duration"].as_f64().unwrap_or(0.0) as u64,
                    src: if sc { "sc".into() } else { String::new() },
                    url: if sc {
                        e["url"].as_str().or(e["webpage_url"].as_str())?.into()
                    } else {
                        String::new()
                    },
                    thumb: if sc {
                        e["thumbnails"]
                            .as_array()
                            .and_then(|a| a.last())
                            .and_then(|t| t["url"].as_str())
                            .unwrap_or("")
                            .into()
                    } else {
                        String::new()
                    },
                    preview: String::new(),
                    artist_id: String::new(),
                },
                chan,
                chan_id: e["channel_id"].as_str().unwrap_or("").into(),
                verified: e["channel_is_verified"].as_bool().unwrap_or(false),
            })
        })
        .collect())
}

/// "Official" = auto-generated Topic channel, verified VEVO channel, or a verified channel whose
/// name matches the artist named in the title or the query. (ponytail: heuristic, not a label-registry lookup)
pub(crate) fn official_yt(r: &Raw, q: &str) -> bool {
    let topic = r.chan.ends_with(" - Topic");
    let vevo = r.verified && r.chan.to_lowercase().ends_with("vevo");
    let k = key(&r.chan);
    topic
        || vevo
        || (r.verified && !k.is_empty() && (norm(&r.t.title).contains(&k) || norm(q).contains(&k)))
}

/// SoundCloud has no verified flag in flat results: accept only an uploader named like the artist, and not 30 s previews.
pub(crate) fn official_sc(r: &Raw, q: &str) -> bool {
    let k = key(&r.chan);
    r.t.secs > 45 && !k.is_empty() && (norm(q).contains(&k) || norm(&r.t.title).contains(&k))
}

/// The optional PO-token provider needs Node and git, so it is Unix-only for now.
#[cfg(unix)]
pub(crate) use pot::{start_pot_provider, stop_pot_provider};
#[cfg(not(unix))]
pub(crate) fn start_pot_provider() {}
#[cfg(not(unix))]
pub(crate) fn stop_pot_provider() {}

#[cfg(unix)]
mod pot {
    use super::*;

    // ── optional PO-token provider (bgutil) ──────────────────────────────────────────────────────────
    // YouTube increasingly wants a "proof of origin" token from clients it does not trust. The bgutil
    // provider is a local HTTP server (127.0.0.1:4416) plus a yt-dlp plugin that fetches tokens from it.
    // It may reduce bot-check errors but does not guarantee it. Needs git, npm and Node >= 22; if any is
    // missing, or SPOTTY_NO_POT is set, the app simply runs without it.

    const POT_VERSION: &str = "2.0.2";
    const POT_ADDR: &str = "127.0.0.1:4416";
    static POT_CHILD: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);

    fn run_ok(cmd: &mut Command) -> Result<(), String> {
        let o = cmd.output().map_err(|e| e.to_string())?;
        if o.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&o.stderr)
                .lines()
                .last()
                .unwrap_or("command failed")
                .to_string())
        }
    }

    fn setup_pot() -> Result<(), String> {
        let dir = cache_dir();
        let plugin = dir.join("bin/yt-dlp-plugins/bgutil-ytdlp-pot-provider.zip");
        let repo = dir.join("bgutil");
        let server = repo.join("server");
        if !plugin.exists() {
            let url = format!(
                "https://github.com/Brainicism/bgutil-ytdlp-pot-provider/releases/download/{POT_VERSION}/bgutil-ytdlp-pot-provider.zip"
            );
            let bytes = ureq::get(&url)
                .call()
                .map_err(|e| e.to_string())?
                .body_mut()
                .read_to_vec()
                .map_err(|e| e.to_string())?;
            std::fs::create_dir_all(plugin.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&plugin, bytes).map_err(|e| e.to_string())?;
        }
        if !server.join("build/main.js").exists() {
            if !repo.exists() {
                run_ok(
                    Command::new("git")
                        .args([
                            "clone",
                            "-q",
                            "--single-branch",
                            "--branch",
                            POT_VERSION,
                            "https://github.com/Brainicism/bgutil-ytdlp-pot-provider.git",
                        ])
                        .arg(&repo),
                )?;
            }
            run_ok(
                Command::new("npm")
                    .args(["ci", "--silent"])
                    .current_dir(&server),
            )?;
            run_ok(Command::new("npx").arg("tsc").current_dir(&server))?;
        }
        if std::net::TcpStream::connect_timeout(
            &POT_ADDR.parse().unwrap(),
            Duration::from_millis(300),
        )
        .is_ok()
        {
            return Ok(()); // someone already runs a provider
        }
        let child = Command::new("node")
            .arg("build/main.js")
            .current_dir(&server)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        *POT_CHILD.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Install (first run) and start the provider in the background; never blocks or fails the app.
    pub(crate) fn start_pot_provider() {
        if std::env::var_os("SPOTTY_NO_POT").is_some() {
            return;
        }
        std::thread::spawn(|| {
            if let Err(e) = setup_pot() {
                eprintln!("PO-token provider unavailable (continuing without it): {e}");
            }
        });
    }

    pub(crate) fn stop_pot_provider() {
        if let Some(mut c) = POT_CHILD.lock().unwrap().take() {
            let _ = c.kill();
        }
    }
}
