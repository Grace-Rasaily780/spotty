use crate::db;
use crate::model::{Msg, Track};
use crate::util::{base_title, key, norm};
use crate::ytdlp::{
    command, is_block_err, official_sc, plain_err, raw_search, set_yt_blocked, yt_blocked,
    ytdlp_err,
};
use eframe::egui::{self};
use std::path::PathBuf;
use std::sync::mpsc;

pub(crate) const CHUNK: u64 = 1 << 20;

pub(crate) fn fetch_range(url: &str, start: u64, end: u64) -> Result<Vec<u8>, String> {
    let mut r = ureq::get(url)
        .header("Range", format!("bytes={start}-{end}"))
        .call()
        .map_err(|e| e.to_string())?;
    r.body_mut().read_to_vec().map_err(|e| e.to_string())
}

/// Read+Seek over HTTP Range requests. Prefetches the next chunk in the background so playback
/// never waits on the network at a chunk boundary. Nothing is written to disk.
pub(crate) struct HttpFile {
    url: String,
    len: u64,
    pos: u64,
    buf: Vec<u8>,
    buf_start: u64,
    pre: Option<(
        u64,
        std::sync::Mutex<mpsc::Receiver<Result<Vec<u8>, String>>>,
    )>,
}

impl HttpFile {
    fn open(url: String) -> Result<Self, String> {
        let mut r = ureq::get(&url)
            .header("Range", "bytes=0-0")
            .call()
            .map_err(|e| e.to_string())?;
        if r.status().as_u16() == 200 {
            // server ignores Range (e.g. short previews): it sent the whole file
            let data = r
                .body_mut()
                .with_config()
                .limit(64 << 20)
                .read_to_vec()
                .map_err(|e| e.to_string())?;
            let len = data.len() as u64;
            return Ok(Self {
                url,
                len,
                pos: 0,
                buf: data,
                buf_start: 0,
                pre: None,
            });
        }
        let len = r
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next()?.parse().ok())
            .ok_or("server gave no length")?;
        Ok(Self {
            url,
            len,
            pos: 0,
            buf: vec![],
            buf_start: 0,
            pre: None,
        })
    }
    fn prefetch(&mut self) {
        let next = self.buf_start + self.buf.len() as u64;
        if next >= self.len {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let (url, end) = (self.url.clone(), (next + CHUNK).min(self.len) - 1);
        std::thread::spawn(move || {
            let _ = tx.send(fetch_range(&url, next, end));
        });
        self.pre = Some((next, std::sync::Mutex::new(rx)));
    }
}

impl std::io::Read for HttpFile {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        if self.pos < self.buf_start || self.pos >= self.buf_start + self.buf.len() as u64 {
            let data = match self.pre.take() {
                Some((s, rx)) if s == self.pos => {
                    rx.lock().unwrap().recv().map_err(std::io::Error::other)?
                }
                _ => fetch_range(&self.url, self.pos, (self.pos + CHUNK).min(self.len) - 1),
            }
            .map_err(std::io::Error::other)?;
            self.buf = data;
            self.buf_start = self.pos;
            self.prefetch();
        }
        let off = (self.pos - self.buf_start) as usize;
        let n = out.len().min(self.buf.len() - off);
        out[..n].copy_from_slice(&self.buf[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl std::io::Seek for HttpFile {
    fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
        use std::io::SeekFrom::*;
        self.pos = match p {
            Start(n) => n,
            Current(d) => (self.pos as i64 + d).max(0) as u64,
            End(d) => (self.len as i64 + d).max(0) as u64,
        };
        Ok(self.pos)
    }
}

pub(crate) fn watch_url(t: &Track) -> String {
    if t.url.is_empty() {
        format!("https://www.youtube.com/watch?v={}", t.id)
    } else {
        t.url.clone()
    }
}

pub(crate) fn resolve(t: &Track) -> Result<String, String> {
    resolve_url(&watch_url(t))
}

pub(crate) fn resolve_url(page: &str) -> Result<String, String> {
    let o = command()?
        .args([
            "-f",
            "bestaudio[ext=m4a]/bestaudio[protocol^=http][protocol!*=m3u8]/http_mp3_1_0",
            "-g",
            "--no-playlist",
            page,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(ytdlp_err(&o));
    }
    Ok(String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string())
}

/// Find the same recording on one video/audio site, accepting only official uploads:
/// YouTube "Artist - Topic" channels or verified channels named like the artist; SoundCloud uploaders named like the artist.
/// Title must match and duration must be within 8 s, so covers, remixes and lyric reuploads are rejected.
pub(crate) fn match_official(prefix: &str, t: &Track) -> Result<Option<String>, String> {
    let q = format!("{} {}", t.artist, t.title);
    let (ka, kt) = (key(&t.artist), norm(base_title(&t.title)));
    Ok(raw_search(prefix, &q, 12)?
        .into_iter()
        .find(|r| {
            let kc = key(&r.chan);
            let by_artist = !ka.is_empty()
                && !kc.is_empty()
                && (kc == ka || kc.contains(&ka) || ka.contains(&kc));
            let official = if prefix == "ytsearch" {
                r.chan.ends_with(" - Topic") || (r.verified && by_artist)
            } else {
                by_artist
            };
            let dur = t.secs == 0 || r.t.secs == 0 || r.t.secs.abs_diff(t.secs) <= 8;
            official && dur && norm(&r.t.title).contains(&kt)
        })
        .map(|r| {
            if prefix == "scsearch" {
                r.t.url
            } else {
                watch_url(&r.t)
            }
        }))
}

pub(crate) fn dec(url: String) -> Result<rodio::Decoder<HttpFile>, String> {
    let f = HttpFile::open(url)?;
    let len = f.len;
    rodio::Decoder::builder()
        .with_data(f)
        .with_byte_len(len)
        .build()
        .map_err(|e| e.to_string())
}

/// Source chain for a catalogue track: remembered match → official YouTube → official SoundCloud → 30 s preview.
/// Returns (decoder, is_preview, optional note for the user).
pub(crate) fn play_catalogue(
    t: &Track,
) -> Result<(rodio::Decoder<HttpFile>, bool, Option<String>), String> {
    let cached = db::resolved(&t.id);
    if let Some(u) = cached {
        if let Ok(d) = resolve_url(&u).and_then(dec) {
            return Ok((d, false, None));
        }
    }
    let mut note = None;
    if yt_blocked() {
        note =
            Some("YouTube is limiting this connection, using other official sources".to_string());
    } else {
        match match_official("ytsearch", t) {
            Ok(Some(u)) => match resolve_url(&u) {
                Ok(url) => match dec(url) {
                    Ok(d) => {
                        db::put_resolved(&t.id, &u);
                        set_yt_blocked(false);
                        return Ok((d, false, None));
                    }
                    Err(_) => {}
                },
                Err(e) if is_block_err(&e) => {
                    set_yt_blocked(true);
                    note = Some(
                        "YouTube is limiting this connection, using other official sources".into(),
                    );
                }
                Err(_) => {}
            },
            Err(e) if is_block_err(&e) => {
                set_yt_blocked(true);
                note = Some(
                    "YouTube is limiting this connection, using other official sources".into(),
                );
            }
            _ => {}
        }
    }
    if let Ok(Some(u)) = match_official("scsearch", t) {
        if let Ok(d) = resolve_url(&u).and_then(dec) {
            db::put_resolved(&t.id, &u);
            return Ok((d, false, note));
        }
    }
    if !t.preview.is_empty() {
        let d = dec(t.preview.clone())?;
        return Ok((
            d,
            true,
            Some("No full-length official stream found. Playing the 30-second preview.".into()),
        ));
    }
    Err("No official source found for this track".into())
}

/// Stream the track (nothing is written to disk).
pub(crate) fn stream(req: usize, t: Track, tx: mpsc::Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let res = (|| -> Result<(rodio::Decoder<HttpFile>, bool), String> {
            if t.src == "dz" {
                let (d, pv, note) = play_catalogue(&t)?;
                if let Some(n) = note {
                    let _ = tx.send(Msg::Toast(n));
                }
                return Ok((d, pv));
            }
            // older saved tracks: direct YouTube / SoundCloud ids
            let url = if t.src.is_empty() && yt_blocked() {
                Err("YouTube blocked".to_string())
            } else {
                resolve(&t)
            };
            let url = match url {
                Ok(u) => {
                    if t.src.is_empty() {
                        set_yt_blocked(false);
                    }
                    u
                }
                Err(e) if t.src.is_empty() => {
                    if is_block_err(&e) {
                        set_yt_blocked(true);
                    }
                    let q = format!("{} {}", t.artist, t.title);
                    let alt = raw_search("scsearch", &q, 8)?
                        .into_iter()
                        .find(|r| official_sc(r, &q) && r.t.secs.abs_diff(t.secs) < 15)
                        .ok_or_else(|| {
                            format!("{} No official copy found elsewhere.", plain_err(&e))
                        })?;
                    resolve(&alt.t)?
                }
                Err(e) => return Err(e),
            };
            Ok((dec(url)?, false))
        })();
        let _ = tx.send(match res {
            Ok((d, pv)) => Msg::Ready(req, t, d, pv),
            Err(e) => Msg::Fail(req, t.id.clone(), e),
        });
        ctx.request_repaint();
    });
}

/// Explicit download only: saves to ~/Music/Spotty.
pub(crate) fn download(t: Track, tx: mpsc::Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let dir = dirs::audio_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Spotty");
        let _ = std::fs::create_dir_all(&dir);
        let o = command().and_then(|mut c| {
            c.args([
                "-f",
                "bestaudio[ext=m4a]/bestaudio[protocol^=http][protocol!*=m3u8]",
                "--no-playlist",
                "-o",
            ])
            .arg(dir.join("%(title)s.%(ext)s"))
            .arg(watch_url(&t))
            .output()
            .map_err(|e| e.to_string())
        });
        let _ = tx.send(Msg::Toast(match o {
            Ok(o) if o.status.success() => format!("Saved “{}” to {}", t.title, dir.display()),
            Ok(o) => plain_err(&ytdlp_err(&o)),
            Err(e) => e.to_string(),
        }));
        ctx.request_repaint();
    });
}
