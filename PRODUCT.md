# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive (Linux and Windows desktop)

## Stack
Rust, eframe/egui 0.37 (native desktop, Linux and Windows), rodio for audio, SQLite (rusqlite) for the library, ureq for HTTP. yt-dlp and a JavaScript runtime are downloaded and checksum-verified on first run (`src/setup.rs`). Modules: catalog, stream, ytdlp, setup, db, ui/*.

## Users
One person listening on their own laptop who wants a Spotify-like player without a Spotify account or service: search artists and songs, build playlists, listen. Desktop, mouse and keyboard; window may be narrow, so layouts collapse.

## Product Purpose
A local-first Spotify clone that streams audio on-device through yt-dlp instead of a music service. Success: search -> play within seconds, feels as polished as the Spotify desktop app, nothing is written to disk unless the user presses download.

## Positioning
Same familiar Spotify experience (sidebar, artist pages, playlists, player bar) with no account, no service lock-in, and playback restricted to official artist sources.

## Operating Context
Streams over HTTP range requests; YouTube can block bot-like traffic (HTTP 429), so SoundCloud is the fallback source and YouTube is re-probed every 5 minutes. Liked songs and playlists persist as JSON in the user cache directory.

## Capabilities and Constraints
- Search tracks and artists; artist page with top songs.
- Playback only from official sources: verified channels, auto-generated "Artist - Topic" channels, verified SoundCloud/Bandcamp accounts. Unofficial uploads are hidden; a track with no official source is shown as unavailable. (Confirmed by user.)
- Playlists: create, rename, delete, add/remove tracks, play.
- Downloads happen only when the user presses download (to ~/Music/Spotty).
- Layout must adapt when the window is narrow (sidebar collapses). (Confirmed by user.)
- Open decision: Bandcamp has no yt-dlp search; it can only be used for direct URLs.

## Brand Commitments
Visual world is pinned by the user: clone Spotify's web look (near-black surfaces, rounded cards, green accent, pill search, round white play button), refined with Apple-style motion. App name "Spotty"; do not use Spotify's logo or marks.

## Evidence on Hand
No artwork or brand assets. Cover art comes from YouTube/SoundCloud thumbnails at runtime.

## Product Principles
1. Feedback is instant and motion is interruptible: press, hover, and load states respond on the same frame.
2. Never show an empty void: every load has a skeleton, every failure has a plain-language message and a way forward.
3. Official sources or nothing: trust beats catalogue size.
4. Familiar over novel: where Spotify has a pattern, use it.
5. Restraint: one accent colour, motion that explains, nothing decorative.

## Accessibility & Inclusion
Respect reduced motion (skeletons become static, no springs). Keep text contrast at least 4.5:1 on dark surfaces; hit targets at least 32 px.
