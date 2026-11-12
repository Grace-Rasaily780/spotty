use crate::model::{Artist, Track};
use crate::util::{fix_url, key, norm, urlenc};
use crate::ytdlp::{official_sc, official_yt, plain_err, raw_search, run_json};
use std::collections::HashSet;

pub(crate) fn dedupe(v: Vec<Track>) -> Vec<Track> {
    let mut seen = HashSet::new();
    v.into_iter()
        .filter(|t| seen.insert((norm(&t.title), norm(&t.artist))))
        .collect()
}

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
        url: String::new(),
        thumb: e["album"]["cover_medium"].as_str().unwrap_or("").into(),
        preview: e["preview"].as_str().unwrap_or("").into(),
        artist_id: e["artist"]["id"]
            .as_u64()
            .map(|i| i.to_string())
            .unwrap_or_default(),
    })
}

pub(crate) fn find_tracks(q: &str) -> Result<Vec<Track>, String> {
    match deezer(&format!("/search?q={}&limit=25", urlenc(q))) {
        Ok(v) => Ok(dedupe(
            v["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(dz_track)
                .collect(),
        )
        .into_iter()
        .take(20)
        .collect()),
        Err(_) => find_tracks_yt(q), // catalogue unreachable: fall back to searching the video sites directly
    }
}

pub(crate) fn find_tracks_yt(q: &str) -> Result<Vec<Track>, String> {
    let yt = raw_search("ytsearch", q, 30).map(|r| {
        r.into_iter()
            .filter(|r| official_yt(r, q))
            .map(|r| r.t)
            .collect::<Vec<_>>()
    });
    match yt {
        Ok(t) => Ok(dedupe(t).into_iter().take(15).collect()),
        Err(e) => {
            let t = raw_search("scsearch", q, 20)
                .map_err(|e2| format!("{} / {}", plain_err(&e), e2))?;
            Ok(dedupe(
                t.into_iter()
                    .filter(|r| official_sc(r, q))
                    .map(|r| r.t)
                    .collect(),
            )
            .into_iter()
            .take(15)
            .collect())
        }
    }
}

pub(crate) fn find_artists_yt(q: &str) -> Result<Vec<Artist>, String> {
    let url = format!(
        "https://www.youtube.com/results?search_query={}&sp=EgIQAg%253D%253D",
        urlenc(q)
    );
    let v = run_json(&["--flat-playlist", "-J", "--playlist-end", "14", &url])?;
    let mut groups: Vec<(String, Artist, bool)> = vec![]; // key, artist, has_verified
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
    Ok(groups.into_iter().map(|g| g.1).take(6).collect())
}

pub(crate) fn artist_tracks_yt(a: &Artist) -> Result<Vec<Track>, String> {
    let raws = raw_search("ytsearch", &a.name, 40)?;
    let t = raws
        .into_iter()
        .filter(|r| a.ids.contains(&r.chan_id))
        .map(|r| r.t)
        .collect();
    Ok(dedupe(t).into_iter().take(20).collect())
}

pub(crate) fn dz_artist(e: &serde_json::Value) -> Option<Artist> {
    Some(Artist {
        name: e["name"].as_str()?.into(),
        thumb: e["picture_xl"]
            .as_str()
            .or(e["picture_big"].as_str())
            .unwrap_or("")
            .into(),
        ids: vec![e["id"].as_u64()?.to_string()],
        followers: e["nb_fan"].as_u64(),
    })
}

/// Artists for a query. `hint` = the song results, so "the 1975 about you" still surfaces The 1975.
pub(crate) fn find_artists(q: &str, hint: &[Track]) -> Result<Vec<Artist>, String> {
    let v = match deezer(&format!("/search/artist?q={}&limit=8", urlenc(q))) {
        Ok(v) => v,
        Err(_) => return find_artists_yt(q),
    };
    let all: Vec<Artist> = v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(dz_artist)
        .collect();
    let kq = norm(q);
    let rel = |a: &Artist| {
        let k = norm(&a.name);
        !k.is_empty() && (kq.contains(&k) || k.contains(&kq))
    };
    let mut out: Vec<Artist> = all.iter().filter(|a| rel(a)).cloned().collect();
    // most frequent artists among the song hits
    let mut counts: Vec<(String, usize)> = vec![];
    for t in hint.iter().filter(|t| !t.artist_id.is_empty()) {
        match counts.iter_mut().find(|c| c.0 == t.artist_id) {
            Some(c) => c.1 += 1,
            None => counts.push((t.artist_id.clone(), 1)),
        }
    }
    // artists named in the query first, then by how many song hits they have
    let named = |id: &str| {
        hint.iter().find(|t| t.artist_id == id).is_some_and(|t| {
            let k = norm(&t.artist);
            !k.is_empty() && kq.contains(&k)
        })
    };
    counts.sort_by_key(|c| (std::cmp::Reverse(named(&c.0)), std::cmp::Reverse(c.1)));
    for (id, _) in counts.into_iter().take(2) {
        if out.iter().any(|a| a.ids[0] == id) {
            continue;
        }
        if let Some(a) = all.iter().find(|a| a.ids[0] == id).cloned().or_else(|| {
            deezer(&format!("/artist/{id}"))
                .ok()
                .and_then(|e| dz_artist(&e))
        }) {
            out.push(a);
        }
    }
    if out.is_empty() {
        out = all;
    }
    // drop tribute/cover "artists": anything under 1% of the biggest match's fans
    let best = out.iter().filter_map(|a| a.followers).max().unwrap_or(0);
    out.retain(|a| a.followers.unwrap_or(0) * 100 >= best);
    Ok(out.into_iter().take(6).collect())
}

pub(crate) fn artist_by_id(id: &str) -> Result<Artist, String> {
    dz_artist(&deezer(&format!("/artist/{id}"))?).ok_or_else(|| "Artist not found".to_string())
}

pub(crate) fn artist_tracks(a: &Artist) -> Result<Vec<Track>, String> {
    match a.ids.first() {
        Some(id) if id.chars().all(|c| c.is_ascii_digit()) => {
            let v = deezer(&format!("/artist/{id}/top?limit=25"))?;
            Ok(dedupe(
                v["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(dz_track)
                    .collect(),
            ))
        }
        _ => artist_tracks_yt(a),
    }
}
