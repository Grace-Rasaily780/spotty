# Spotty

A desktop music player that looks and works like Spotify, but streams on your own machine:
search songs and artists, build playlists, and play from **official artist sources only**.
No account, no service to run.

## Install

```sh
tar xzf spotty-*-linux-*.tar.gz && cd spotty-*/
./install.sh          # installs to ~/.local/bin and adds an app-menu entry
```

First launch downloads two helpers (yt-dlp and a JavaScript runtime, about 80 MB, checksum-verified)
into `~/.cache/spotty`. A banner shows progress; after that it works offline-first and keeps yt-dlp
updated about once a day. Needs Linux (x86_64 or aarch64) and an audio output device.

## How playback works

1. Search uses the Deezer catalogue (artists, albums, covers, durations).
2. For each track Spotty looks for the matching **official** upload: a YouTube "Artist - Topic" channel,
   a verified channel named like the artist, or a SoundCloud uploader named like the artist.
   Title and length must match, so covers, remixes and lyric re-uploads are rejected.
3. If YouTube blocks the connection, Spotty uses the other sources and retries YouTube every 5 minutes.
4. If no full-length official stream exists, the 30-second preview plays and the player says so.

Audio is streamed in chunks and never saved. Press the download button on a track to save it to `~/Music/Spotty`.

## Your data

- Library (liked songs, playlists, history): `~/.local/share/spotty/spotty.db` (SQLite). Back this file up.
- Downloaded helpers and caches: `~/.cache/spotty` (safe to delete; they are fetched again).

## Settings (environment variables)

| Variable | Effect |
|---|---|
| `SPOTTY_COOKIES=firefox` | Give yt-dlp your browser's YouTube session (helps if YouTube keeps asking to sign in). Use a throwaway account. |
| `SPOTTY_REDUCE_MOTION=1` | No fades, shimmer sweeps or spinners animation. |
| `SPOTTY_NO_POT=1` | Do not start the optional token provider. |

## Troubleshooting

- **"Setup failed" banner:** check your connection and press Retry.
- **A track says "Unavailable":** no official source exists for it right now.
- **Preview only:** the artist has no official full-length upload on a supported site.
- `spotty --probe "artist song"` prints how a search would be sourced, for bug reports.

## Build from source

```sh
cargo build --release      # binary: target/release/spotty
cargo test --release
scripts/package.sh         # release tarball in dist/
```

## Legal

Spotty only streams what the sites it connects to make publicly available. Using it may be subject
to those sites' terms of service; that is your responsibility. It is not affiliated with Spotify.
