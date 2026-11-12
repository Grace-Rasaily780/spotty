use crate::stream::HttpFile;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub(crate) struct Track {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) secs: u64,
    #[serde(default)]
    pub(crate) src: String, // "" = youtube, "sc" = soundcloud
    #[serde(default)]
    pub(crate) url: String,
    #[serde(default)]
    pub(crate) thumb: String,
    #[serde(default)]
    pub(crate) preview: String, // 30 s preview url (catalogue tracks)
    #[serde(default)]
    pub(crate) artist_id: String, // catalogue artist id
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub(crate) struct Playlist {
    pub(crate) name: String,
    pub(crate) tracks: Vec<Track>,
}

#[derive(Clone, Default)]
pub(crate) struct Artist {
    pub(crate) name: String,
    pub(crate) thumb: String,
    pub(crate) ids: Vec<String>,
    pub(crate) followers: Option<u64>,
}

pub(crate) enum Msg {
    Tracks(u64, Result<Vec<Track>, String>),
    Artists(u64, Result<Vec<Artist>, String>),
    ArtistPage(u64, Result<(Artist, Vec<Track>), String>),
    Ready(usize, Track, rodio::Decoder<HttpFile>, bool),
    Fail(usize, String, String),
    Toast(String),
}

#[derive(Clone, PartialEq)]
pub(crate) enum View {
    Home,
    Search,
    Liked,
    Artist,
    Playlist(usize),
}
