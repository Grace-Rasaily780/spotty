# Spotty

A desktop music player that looks and works like Spotify, but streams on your own machine:
search songs and artists, build playlists, and play from **official artist sources only**.
No account, no service to run.

## Install

Download the file for your system from the **Releases** page.

**Windows:** unzip `spotty-*-windows-x86_64.zip` and run `spotty.exe`. The executable is not code-signed,
so Windows SmartScreen may warn the first time: choose "More info" then "Run anyway".

**Linux:**
```sh
tar xzf spotty-*-linux-*.tar.gz && cd spotty-*/
./install.sh          # installs to ~/.local/bin and adds an app-menu entry
```

First launch downloads two helpers (yt-dlp and a JavaScript runtime, about 80 MB to download and 140 MB on disk, checksum-verified)
into the cache folder. A banner shows progress; afterwards it keeps yt-dlp updated about once a day.
Needs Windows 10+ or Linux (x86_64; aarch64 builds from source) and an audio output device.

## How playback works

1. Search is YouTube-first. Two searches are merged and ranked: official sources first, then relevance,
   with live cuts, remixes, sped-up edits and mixes pushed down. Titles and artists are cleaned up
   ("The 1975 - About You (Official Video)" becomes "About You" by The 1975). Every result is a real,
   playable upload.
2. **Official only** (on by default, next to the search box) keeps YouTube "Artist - Topic" channels, verified
   VEVO channels and verified channels named like the artist you searched for. Turn it off to include other
   uploads; they are ranked lower and marked "unofficial upload".
3. Artist pages come from the artist's official YouTube channels.
4. If YouTube blocks the connection, Spotty looks for an official SoundCloud copy and retries YouTube
   every 5 minutes.
5. If no full-length stream can be played, a 30-second preview from the Deezer catalogue plays and the
   player says so.

Audio is streamed in chunks and never saved. Press the download button on a track to save it to your
`Music/Spotty` folder.

## Your data

- Library (liked songs, playlists, history): `spotty.db` (SQLite) in your user data folder
  (`~/.local/share/spotty` on Linux, `%APPDATA%\spotty` on Windows). Back this file up.
- Downloaded helpers and caches: `~/.cache/spotty` on Linux, `%LOCALAPPDATA%\spotty` on Windows
  (safe to delete; they are fetched again).

## Settings (environment variables)

| Variable | Effect |
|---|---|
| `SPOTTY_COOKIES=firefox` | Give yt-dlp your browser's YouTube session (helps if YouTube keeps asking to sign in). Use a throwaway account. |
| `SPOTTY_REDUCE_MOTION=1` | No fades, shimmer sweeps or spinners animation. |
| `SPOTTY_NO_POT=1` | Turn off the built-in YouTube token provider (on by default, downloaded on first run). |

## Troubleshooting

- **"Setup failed" banner:** check your connection and press Retry.
- **A track says "Unavailable":** no official source exists for it right now.
- **Preview only:** the artist has no official full-length upload on a supported site.
- `spotty --probe "artist song"` prints how a search would be sourced, for bug reports.

## Build from source

```sh
cargo build --release      # binary: target/release/spotty
cargo test --release
scripts/package.sh         # Linux release tarball in dist/
```

## Releasing

Set `version` in `Cargo.toml`, then push a matching tag. GitHub Actions builds, tests and publishes the
Linux and Windows downloads on the Releases page:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

Fonts: Noto Sans (Apache-2.0), see `assets/fonts`.

## Legal

Spotty only streams what the sites it connects to make publicly available. Using it may be subject
to those sites' terms of service; that is your responsibility. It is not affiliated with Spotify.
