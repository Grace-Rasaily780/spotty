//! Search: songs and artists come from YouTube (ranked), with SoundCloud as a fallback.
//! Every listed song is a real, playable upload. Deezer is used only to find a 30-second
//! preview when no full-length stream can be played (see `stream.rs`).

use crate::model::{Artist, Track};
use crate::rank::rank_tracks;
use crate::util::{fix_url, key, norm, urlenc};
use crate::ytdlp::{Raw, official_sc, plain_err, raw_search, run_json};
use std::collections::HashSet;

pub(crate) fn dedupe(v: Vec<Track>) -> Vec<Track> {
    let mut seen = HashSet::new();
    v.into_iter()
        .filter(|t| seen.insert((norm(&t.title), norm(&t.artist))))
        .collect()
}

// ── Deezer: metadata for 30 s previews, and for songs saved by older versions ──

pub(crate) fn deezer(path: &str) -> Result<serde_json::Value, String> {
    let mut r = ureq::get(&format!("https://api.deezer.com{path}"))
        .call()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value =
        serde_json::from_str(&r.body_mut().read_to_string().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if let Some(e) = v.get("error") {
        return Err(e["message"]
            .as_str()
            .unwrap_or("catalogue error")
            .to_string());
    }
    Ok(v)
}

pub(crate) fn dz_track(e: &serde_json::Value) -> Option<Track> {
    Some(Track {
        id: format!("dz:{}", e["id"].as_u64()?),
        title: e["title"].as_str()?.into(),
        artist: e["artist"]["name"].as_str().unwrap_or("").into(),
        secs: e["duration"].as_u64().unwrap_or(0),
        src: "dz".into(),
        thumb: e["album"]["cover_medium"].as_str().unwrap_or("").into(),
        preview: e["preview"].as_str().unwrap_or("").into(),
        artist_id: e["artist"]["id"]
            .as_u64()
            .map(|i| i.to_string())
            .unwrap_or_default(),
        ..Default::default()
    })
}

/// 30-second preview URL for `t` from the Deezer catalogue (title must match, length within 8 s).
pub(crate) fn deezer_preview(t: &Track) -> Option<String> {
    let q = format!("{} {}", t.artist, t.title);
    let v = deezer(&format!("/search?q={}&limit=5", urlenc(&q))).ok()?;
    let want = norm(crate::util::base_title(&t.title));
    v["data"]
        .as_array()?
        .iter()
        .filter_map(dz_track)
        .find(|d| {
            norm(&d.title).contains(&want)
                && (t.secs == 0 || d.secs == 0 || d.secs.abs_diff(t.secs) <= 8)
                && !d.preview.is_empty()
        })
        .map(|d| d.preview)
}

// ── YouTube-first search ──

/// Songs for `q`: two YouTube searches (plain, and biased to studio/official audio) merged and
/// ranked. If YouTube cannot be searched at all, SoundCloud is used instead.
pub(crate) fn find_tracks(q: &str, official_only: bool) -> Result<Vec<Track>, String> {
    let (plain, official) = std::thread::scope(|s| {
        let a = s.spawn(|| raw_search("ytsearch", q, 30));
        let b = raw_search("ytsearch", &format!("{q} official audio"), 20);
        (a.join().unwrap_or_else(|_| Err("search crashed".into())), b)
    });
    let mut raws: Vec<Raw> = vec![];
    let mut last_err = None;
    for r in [plain, official] {
        match r {
            Ok(v) => raws.extend(v),
            Err(e) => last_err = Some(e),
        }
    }
    match (raws.is_empty(), last_err) {
        (false, _) => Ok(rank_tracks(raws, q, official_only, 20)),
        (true, None) => Ok(vec![]),
        (true, Some(e)) => {
            let sc = raw_search("scsearch", q, 20)
                .map_err(|e2| format!("{} / {}", plain_err(&e), e2))?;
            Ok(dedupe(
                sc.into_iter()
                    .filter(|r| official_sc(r, q))
                    .map(|r| r.t)
                    .collect(),
            )
            .into_iter()
            .take(20)
            .collect())
        }
    }
}

/// Artists for `q`: official YouTube channels (verified or "Topic"), merged per artist.
pub(crate) fn find_artists(q: &str) -> Result<Vec<Artist>, String> {
    let url = format!(
        "https://www.youtube.com/results?search_query={}&sp=EgIQAg%253D%253D",
        urlenc(q)
    );
    let v = run_json(&["--flat-playlist", "-J", "--playlist-end", "14", &url])?;
    let mut groups: Vec<(String, Artist, bool)> = vec![]; // key, artist, has verified main channel
    for e in v["entries"].as_array().into_iter().flatten() {
        let (Some(name), Some(id)) = (e["title"].as_str(), e["id"].as_str()) else {
            continue;
        };
        let verified = e["channel_is_verified"].as_bool().unwrap_or(false);
        let topic = name.ends_with(" - Topic");
        if !(verified || topic) {
            continue;
        }
        let thumb = e["thumbnails"]
            .as_array()
            .and_then(|a| a.last())
            .and_then(|t| t["url"].as_str())
            .map(fix_url)
            .unwrap_or_default();
        let clean = name.trim_end_matches(" - Topic").to_string();
        let k = key(name);
        let plain = !topic && !name.to_lowercase().ends_with("vevo");
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => {
                g.1.ids.push(id.into());
                if plain && verified {
                    g.1.name = clean;
                    if !thumb.is_empty() {
                        g.1.thumb = thumb;
                    }
                    g.2 = true;
                    g.1.followers = e["channel_follower_count"].as_u64().or(g.1.followers);
                }
            }
            None => groups.push((
                k,
                Artist {
                    name: clean,
                    thumb,
                    ids: vec![id.into()],
                    followers: e["channel_follower_count"].as_u64(),
                },
                verified && plain,
            )),
        }
    }
    groups.sort_by_key(|g| !g.2); // verified artist channels first (stable)
    // artists the query actually names; only if none does, fall back to the top channels
    let kq = norm(q);
    let named = |a: &Artist| {
        let k = key(&a.name);
        !k.is_empty() && (kq.contains(&k) || k.contains(&kq))
    };
    let (named_artists, others): (Vec<_>, Vec<_>) =
        groups.into_iter().map(|g| g.1).partition(named);
    let list = if named_artists.is_empty() {
        others
    } else {
        named_artists
    };
    Ok(list.into_iter().take(6).collect())
}

/// Popular songs of an artist: ranked search results that come from the artist's own channels.
pub(crate) fn artist_tracks(a: &Artist) -> Result<Vec<Track>, String> {
    let raws = raw_search("ytsearch", &a.name, 40)?;
    let own: Vec<Raw> = raws
        .into_iter()
        .filter(|r| a.ids.contains(&r.chan_id))
        .collect();
    Ok(rank_tracks(own, &a.name, true, 20))
}
