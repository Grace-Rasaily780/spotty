//! Ranking and tidying of video-site search results so the list reads like a music catalogue:
//! official uploads first, noise (live cuts, remixes, sped-up edits, mixes) pushed down,
//! and titles/artists cleaned ("The 1975 - About You (Official Video)" -> "About You" by "The 1975").

use crate::model::Track;
use crate::util::{key, norm};
use crate::ytdlp::{Raw, official_yt};
use std::collections::HashSet;

/// Words that usually mean "not the studio recording", unless the user typed them.
const NOISE: &[&str] = &[
    "live",
    "remix",
    "cover",
    "karaoke",
    "sped up",
    "slowed",
    "reverb",
    "nightcore",
    "8d",
    "reaction",
    "tutorial",
    "instrumental",
    "full album",
    "playlist",
    "mix",
    "1 hour",
    "loop",
];

/// Bracketed tags that are decoration, not part of the song title.
const TAGS: &[&str] = &[
    "official",
    "video",
    "audio",
    "lyric",
    "lyrics",
    "visualizer",
    "visualiser",
    "hd",
    "hq",
    "4k",
    "remaster",
    "mv",
    "music video",
];

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn has_phrase(hay: &[String], phrase: &str) -> bool {
    let p = words(phrase);
    !p.is_empty() && hay.windows(p.len()).any(|w| w == p.as_slice())
}

/// Higher is better. Trust dominates, then query relevance, then penalties and popularity.
pub(crate) fn score(r: &Raw, q: &str) -> i32 {
    let (title, query, chan) = (words(&r.t.title), words(q), words(&r.chan));
    let k = key(&r.chan);
    // the channel is the artist the user asked for, vs. a channel that merely appears in the title (MTV, Vogue...)
    let in_query = !k.is_empty() && norm(q).contains(&k);
    let in_title = !k.is_empty() && norm(&r.t.title).contains(&k);
    let vevo = r.chan.to_lowercase().ends_with("vevo");
    let mut s = match () {
        _ if r.chan.ends_with(" - Topic") => 60,
        _ if r.verified && (vevo || in_query) => 45,
        _ if r.verified && in_title => 20,
        _ if r.verified => 15,
        _ if in_query => 10,
        _ => 0,
    };
    if !query.is_empty() {
        let hit = query
            .iter()
            .filter(|w| title.contains(w) || chan.contains(w))
            .count();
        s += (hit * 40 / query.len()) as i32;
    }
    for n in NOISE {
        if has_phrase(&title, n) && !has_phrase(&query, n) {
            s -= 25;
        }
    }
    s += match r.t.secs {
        0 => -10,
        1..=59 => -30,
        60..=600 => 0,
        601..=900 => -20,
        _ => -60,
    };
    if r.views > 0 {
        s += ((r.views as f64).log10() * 2.0).min(16.0) as i32;
    }
    s
}

/// Remove decorative bracket tags: "About You (Official Video) [HD]" -> "About You".
/// Brackets that carry meaning ("(feat. X)", "(Live at MSG)") stay.
pub(crate) fn strip_tags(title: &str) -> String {
    let mut out = String::new();
    let mut chars = title.chars().peekable();
    while let Some(c) = chars.next() {
        let close = match c {
            '(' => ')',
            '[' => ']',
            _ => {
                out.push(c);
                continue;
            }
        };
        let mut inner = String::new();
        let mut closed = false;
        for d in chars.by_ref() {
            if d == close {
                closed = true;
                break;
            }
            inner.push(d);
        }
        let low = inner.to_lowercase();
        let decoration = TAGS.iter().any(|t| has_phrase(&words(&low), t));
        if !decoration {
            out.push(c);
            out.push_str(&inner);
            if closed {
                out.push(close);
            }
        }
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(" -")
        .trim()
        .to_string()
}

/// Clean title and artist. Official channels are named after the artist, so "Artist - Title"
/// loses its prefix; uploads from other channels get the artist taken from the title.
fn tidy(r: Raw) -> Track {
    let mut t = r.t;
    t.artist = r.chan.trim_end_matches(" - Topic").to_string();
    if let Some((left, right)) = t.title.split_once(" - ") {
        let (left, right) = (left.trim(), right.trim());
        let same_artist = !key(&t.artist).is_empty() && norm(left).contains(&key(&t.artist));
        if same_artist {
            t.title = right.to_string();
        } else if left.chars().count() <= 40 && !right.is_empty() {
            t.artist = left.to_string();
            t.title = right.to_string();
        }
    }
    t.title = strip_tags(&t.title);
    t.artist_id = r.chan_id; // lets a click on the artist open the matching artist page
    t
}

/// Rank, de-duplicate and tidy `raws`. With `official_only`, unofficial uploads are dropped;
/// otherwise they are kept below official ones and flagged `unofficial`.
pub(crate) fn rank_tracks(
    raws: Vec<Raw>,
    q: &str,
    official_only: bool,
    limit: usize,
) -> Vec<Track> {
    let mut seen_ids = HashSet::new();
    let mut scored: Vec<(i32, bool, Track)> = raws
        .into_iter()
        .filter(|r| seen_ids.insert(r.t.id.clone()))
        .map(|r| (score(&r, q), official_yt(&r, q), r))
        .filter(|(_, official, _)| *official || !official_only)
        .map(|(s, official, r)| (s, official, tidy(r)))
        .collect();
    scored.sort_by_key(|x| std::cmp::Reverse(x.0));
    let mut seen = HashSet::new();
    scored
        .into_iter()
        .filter(|(_, _, t)| seen.insert((norm(&t.title), norm(&t.artist))))
        .take(limit)
        .map(|(_, official, mut t)| {
            t.unofficial = !official;
            t
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(title: &str, chan: &str, verified: bool, secs: u64, views: u64) -> Raw {
        Raw {
            t: Track {
                id: format!("{title}{chan}"),
                title: title.into(),
                secs,
                ..Default::default()
            },
            chan: chan.into(),
            chan_id: "UC1".into(),
            verified,
            views,
        }
    }

    #[test]
    fn tags_are_stripped_but_meaning_stays() {
        assert_eq!(strip_tags("About You (Official Video) [HD]"), "About You");
        assert_eq!(strip_tags("Song (feat. Someone)"), "Song (feat. Someone)");
        assert_eq!(strip_tags("Song (Live at MSG)"), "Song (Live at MSG)");
        assert_eq!(strip_tags("Song (Official Audio) -"), "Song");
    }

    #[test]
    fn titles_and_artists_are_tidied() {
        let t = tidy(raw(
            "The 1975 - About You (Official)",
            "The 1975",
            true,
            326,
            1,
        ));
        assert_eq!(
            (t.title.as_str(), t.artist.as_str()),
            ("About You", "The 1975")
        );
        let t = tidy(raw(
            "Adele - Hello (Lyrics)",
            "SomeFanChannel",
            false,
            300,
            1,
        ));
        assert_eq!((t.title.as_str(), t.artist.as_str()), ("Hello", "Adele"));
        let t = tidy(raw("About You", "The 1975 - Topic", false, 326, 1));
        assert_eq!(
            (t.title.as_str(), t.artist.as_str()),
            ("About You", "The 1975")
        );
    }

    #[test]
    fn ranking_prefers_official_and_penalises_noise() {
        let q = "the 1975 about you";
        let topic = raw("About You", "The 1975 - Topic", false, 326, 5_000_000);
        let official = raw(
            "The 1975 - About You (Official)",
            "The 1975",
            true,
            326,
            90_000_000,
        );
        let sped = raw(
            "The 1975 - About You (sped up)",
            "randomuser",
            false,
            200,
            9_000_000,
        );
        let mix = raw(
            "The 1975 best songs mix",
            "randomuser",
            false,
            4000,
            9_000_000,
        );
        assert!(score(&topic, q) > score(&sped, q));
        assert!(score(&official, q) > score(&sped, q));
        assert!(score(&sped, q) > score(&mix, q));
        // typing the noise word cancels its penalty
        assert!(score(&sped, "the 1975 about you sped up") > score(&sped, q));
    }

    #[test]
    fn official_only_filters_and_flags() {
        let q = "the 1975 about you";
        let mk = || {
            vec![
                raw("The 1975 - About You (Official)", "The 1975", true, 326, 1),
                raw("About You (lyrics)", "randomuser", false, 326, 1),
            ]
        };
        let strict = rank_tracks(mk(), q, true, 10);
        assert_eq!(strict.len(), 1);
        assert!(!strict[0].unofficial);
        let all = rank_tracks(mk(), q, false, 10);
        assert_eq!(all.len(), 2);
        assert!(!all[0].unofficial && all[1].unofficial); // official ranks first
    }
}
