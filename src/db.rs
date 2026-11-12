//! SQLite persistence: liked songs, playlists, recently played and the resolved-source cache.
//!
//! One connection behind a global mutex (the UI thread and the streaming threads both read and write).
//! Lists are small, so each save replaces a whole list inside a single transaction.

use crate::model::{Playlist, Track};
use rusqlite::{Connection, Transaction, params};
use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

/// Persistent user data (the library database): `~/.local/share/spotty`.
pub(crate) fn data_dir() -> PathBuf {
    let d = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("spotty");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Re-downloadable helper programs (yt-dlp, JS runtime): `~/.cache/spotty`. Safe for the OS to clear.
pub(crate) fn cache_dir() -> PathBuf {
    let d = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("spotty");
    let _ = std::fs::create_dir_all(&d);
    d
}

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS tracks (
    id TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL, secs INTEGER NOT NULL,
    src TEXT NOT NULL, url TEXT NOT NULL, thumb TEXT NOT NULL, preview TEXT NOT NULL, artist_id TEXT NOT NULL,
    unofficial INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS liked  (track_id TEXT PRIMARY KEY REFERENCES tracks(id), pos INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS recent (track_id TEXT PRIMARY KEY REFERENCES tracks(id), pos INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS playlists (id INTEGER PRIMARY KEY, name TEXT NOT NULL, pos INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks(id),
    pos INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, pos)
);
CREATE TABLE IF NOT EXISTS resolved (track_id TEXT PRIMARY KEY, page_url TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

const TRACK_COLS: &str =
    "t.id, t.title, t.artist, t.secs, t.src, t.url, t.thumb, t.preview, t.artist_id, t.unofficial";

pub(crate) struct Db(Connection);

static DB: LazyLock<Mutex<Db>> = LazyLock::new(|| {
    let path = data_dir().join("spotty.db");
    migrate_cache_db(&path);
    let mut db = Db::open(&path).unwrap_or_else(|e| {
        // never lose the whole app to a damaged file: keep it aside and start fresh
        eprintln!("library database unusable ({e}); moving it aside");
        let _ = std::fs::rename(&path, path.with_extension("db.corrupt"));
        Db::open(&path).expect("cannot create a new library database")
    });
    db.import_json(&cache_dir());
    Mutex::new(db)
});

/// Earlier builds kept the database in the cache directory, which the OS may clear: copy it over once.
fn migrate_cache_db(to: &Path) {
    let from = cache_dir().join("spotty.db");
    if to.exists() || !from.exists() {
        return;
    }
    for suffix in ["", "-wal", "-shm"] {
        let src = PathBuf::from(format!("{}{suffix}", from.display()));
        let dst = PathBuf::from(format!("{}{suffix}", to.display()));
        if src.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
    }
}

fn upsert(tx: &Transaction, t: &Track) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO tracks (id, title, artist, secs, src, url, thumb, preview, artist_id, unofficial)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET title = excluded.title, artist = excluded.artist, secs = excluded.secs,
            src = excluded.src, url = excluded.url, thumb = excluded.thumb, preview = excluded.preview,
            artist_id = excluded.artist_id, unofficial = excluded.unofficial",
        params![t.id, t.title, t.artist, t.secs as i64, t.src, t.url, t.thumb, t.preview, t.artist_id, t.unofficial as i64],
    )?;
    Ok(())
}

fn track_from_row(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        secs: r.get::<_, i64>(3)? as u64,
        src: r.get(4)?,
        url: r.get(5)?,
        thumb: r.get(6)?,
        preview: r.get(7)?,
        artist_id: r.get(8)?,
        unofficial: r.get::<_, i64>(9)? != 0,
    })
}

impl Db {
    fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    fn init(c: Connection) -> rusqlite::Result<Self> {
        c.execute_batch(SCHEMA)?;
        // databases created before the `unofficial` flag existed
        let has: i64 = c.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tracks') WHERE name = 'unofficial'",
            [],
            |r| r.get(0),
        )?;
        if has == 0 {
            c.execute_batch("ALTER TABLE tracks ADD COLUMN unofficial INTEGER NOT NULL DEFAULT 0")?;
        }
        Ok(Self(c))
    }

    /// Ordered tracks of a list table (`liked` or `recent`).
    fn list(&self, table: &str) -> rusqlite::Result<Vec<Track>> {
        let mut q = self.0.prepare(&format!(
            "SELECT {TRACK_COLS} FROM {table} l JOIN tracks t ON t.id = l.track_id ORDER BY l.pos"
        ))?;
        let rows = q.query_map([], track_from_row)?;
        rows.collect()
    }

    /// Replace a whole list table. `table` is always a compile-time constant.
    fn put_list(&mut self, table: &str, tracks: &[Track]) -> rusqlite::Result<()> {
        let tx = self.0.transaction()?;
        for t in tracks {
            upsert(&tx, t)?;
        }
        tx.execute(&format!("DELETE FROM {table}"), [])?;
        for (i, t) in tracks.iter().enumerate() {
            tx.execute(
                &format!("INSERT OR IGNORE INTO {table} (track_id, pos) VALUES (?1, ?2)"),
                params![t.id, i as i64],
            )?;
        }
        tx.commit()
    }

    fn playlists(&self) -> rusqlite::Result<Vec<Playlist>> {
        let mut pls = self
            .0
            .prepare("SELECT id, name FROM playlists ORDER BY pos")?;
        let mut tracks = self.0.prepare(&format!(
            "SELECT {TRACK_COLS} FROM playlist_tracks p JOIN tracks t ON t.id = p.track_id
             WHERE p.playlist_id = ?1 ORDER BY p.pos"
        ))?;
        let heads: Vec<(i64, String)> = pls
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        heads
            .into_iter()
            .map(|(id, name)| {
                let tracks = tracks
                    .query_map([id], track_from_row)?
                    .collect::<Result<_, _>>()?;
                Ok(Playlist { name, tracks })
            })
            .collect()
    }

    fn put_playlists(&mut self, pls: &[Playlist]) -> rusqlite::Result<()> {
        let tx = self.0.transaction()?;
        tx.execute("DELETE FROM playlists", [])?; // cascades to playlist_tracks
        for (i, p) in pls.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlists (name, pos) VALUES (?1, ?2)",
                params![p.name, i as i64],
            )?;
            let id = tx.last_insert_rowid();
            for (j, t) in p.tracks.iter().enumerate() {
                upsert(&tx, t)?;
                tx.execute(
                    "INSERT INTO playlist_tracks (playlist_id, track_id, pos) VALUES (?1, ?2, ?3)",
                    params![id, t.id, j as i64],
                )?;
            }
        }
        tx.commit()
    }

    fn setting(&self, key: &str) -> Option<String> {
        self.0
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .ok()
    }

    fn put_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.0.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    fn resolved(&self, track_id: &str) -> Option<String> {
        self.0
            .query_row(
                "SELECT page_url FROM resolved WHERE track_id = ?1",
                [track_id],
                |r| r.get(0),
            )
            .ok()
    }

    fn put_resolved(&self, track_id: &str, page_url: &str) -> rusqlite::Result<()> {
        self.0.execute(
            "INSERT INTO resolved (track_id, page_url) VALUES (?1, ?2)
             ON CONFLICT(track_id) DO UPDATE SET page_url = excluded.page_url",
            params![track_id, page_url],
        )?;
        Ok(())
    }

    /// One-time import of the old JSON files (`PRAGMA user_version` 0 -> 1), kept as `*.json.bak`.
    fn import_json(&mut self, dir: &Path) {
        let version: i64 = self
            .0
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(1);
        if version != 0 {
            return;
        }
        let read = |name: &str| std::fs::read(dir.join(name)).ok();
        let tracks = |name: &str| {
            read(name)
                .and_then(|b| serde_json::from_slice::<Vec<Track>>(&b).ok())
                .unwrap_or_default()
        };
        let results = [
            self.put_list("liked", &tracks("liked.json")),
            self.put_list("recent", &tracks("recent.json")),
            self.put_playlists(
                &read("playlists.json")
                    .and_then(|b| serde_json::from_slice::<Vec<Playlist>>(&b).ok())
                    .unwrap_or_default(),
            ),
        ];
        if let Some(map) = read("resolved.json").and_then(|b| {
            serde_json::from_slice::<std::collections::HashMap<String, String>>(&b).ok()
        }) {
            for (id, url) in map {
                let _ = self.put_resolved(&id, &url);
            }
        }
        if results.iter().all(Result::is_ok) {
            let _ = self.0.execute_batch("PRAGMA user_version = 1");
            for n in ["liked", "recent", "playlists", "resolved"] {
                let _ = std::fs::rename(
                    dir.join(format!("{n}.json")),
                    dir.join(format!("{n}.json.bak")),
                );
            }
        }
    }
}

/// Run a query on the shared connection. Errors are logged, never fatal: the UI keeps working in memory.
fn with<T: Default>(f: impl FnOnce(&mut Db) -> rusqlite::Result<T>) -> T {
    let mut db = DB.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut db).unwrap_or_else(|e| {
        eprintln!("database error: {e}");
        T::default()
    })
}

pub(crate) fn liked() -> Vec<Track> {
    with(|d| d.list("liked"))
}
pub(crate) fn save_liked(v: &[Track]) {
    with(|d| d.put_list("liked", v))
}
pub(crate) fn recent() -> Vec<Track> {
    with(|d| d.list("recent"))
}
pub(crate) fn save_recent(v: &[Track]) {
    with(|d| d.put_list("recent", v))
}
pub(crate) fn playlists() -> Vec<Playlist> {
    with(|d| d.playlists())
}
pub(crate) fn save_playlists(v: &[Playlist]) {
    with(|d| d.put_playlists(v))
}
pub(crate) fn setting(key: &str) -> Option<String> {
    with(|d| Ok(d.setting(key)))
}

pub(crate) fn put_setting(key: &str, value: &str) {
    with(|d| d.put_setting(key, value))
}

pub(crate) fn resolved(track_id: &str) -> Option<String> {
    with(|d| Ok(d.resolved(track_id)))
}
pub(crate) fn put_resolved(track_id: &str, page_url: &str) {
    with(|d| d.put_resolved(track_id, page_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: format!("Title {id}"),
            ..Default::default()
        }
    }

    #[test]
    fn lists_playlists_and_cache_roundtrip() {
        let mut db = Db::init(Connection::open_in_memory().unwrap()).unwrap();

        db.put_list("liked", &[track("a"), track("b")]).unwrap();
        let ids = |v: Vec<Track>| v.into_iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(db.list("liked").unwrap()), ["a", "b"]);
        db.put_list("liked", &[track("b")]).unwrap();
        assert_eq!(ids(db.list("liked").unwrap()), ["b"]);

        let pls = vec![
            Playlist {
                name: "mix".into(),
                tracks: vec![track("a"), track("a"), track("c")],
            },
            Playlist {
                name: "empty".into(),
                tracks: vec![],
            },
        ];
        db.put_playlists(&pls).unwrap();
        let got = db.playlists().unwrap();
        assert_eq!(
            got.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["mix", "empty"]
        );
        assert_eq!(ids(got[0].tracks.clone()), ["a", "a", "c"]);
        db.put_playlists(&[]).unwrap();
        assert!(db.playlists().unwrap().is_empty());

        assert_eq!(db.setting("official_only"), None);
        db.put_setting("official_only", "0").unwrap();
        assert_eq!(db.setting("official_only").as_deref(), Some("0"));
        let mut flagged = track("z");
        flagged.unofficial = true;
        db.put_list("recent", &[flagged]).unwrap();
        assert!(db.list("recent").unwrap()[0].unofficial);

        assert_eq!(db.resolved("a"), None);
        db.put_resolved("a", "u1").unwrap();
        db.put_resolved("a", "u2").unwrap();
        assert_eq!(db.resolved("a").as_deref(), Some("u2"));
    }
}
