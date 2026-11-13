//! PO-token provider (bgutil): YouTube wants a "proof of origin" token from clients it does not
//! trust. A small local server (127.0.0.1:4416) mints them and a yt-dlp plugin fetches them.
//! Both parts are downloaded on first run, pinned to a SHA-256, and the server is stopped on exit.

use crate::setup::{bin_dir, download};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

const ADDR: &str = "127.0.0.1:4416";
const SERVER: &str =
    "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs/releases/download/v0.8.1";
const PLUGIN: (&str, &str) = (
    "https://github.com/Brainicism/bgutil-ytdlp-pot-provider/releases/download/2.0.2/bgutil-ytdlp-pot-provider.zip",
    "a26b980bf6d8e8400d8f21445daa4d8381bf6183d20f0be309391f86b31e27ef",
);
static CHILD: Mutex<Option<Child>> = Mutex::new(None);

/// (release asset, sha256, executable suffix) for this machine.
fn server_asset() -> Result<(&'static str, &'static str, &'static str), String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok((
            "bgutil-pot-linux-x86_64",
            "e7c264a574fa2705b6e5dc62283a8a4e80130f27b9d7e9df44e6b09aa6151a87",
            "",
        )),
        ("linux", "aarch64") => Ok((
            "bgutil-pot-linux-aarch64",
            "4f4a1f681dba45e695e1c14d314517da180a1fd374afd09d634fd80ef6d0284b",
            "",
        )),
        ("windows", "x86_64") => Ok((
            "bgutil-pot-windows-x86_64.exe",
            "25d6b05c79176aa792454c3d1727922ca47e56cf11cb1e866615d751819b14a0",
            ".exe",
        )),
        (os, arch) => Err(format!("no token provider for {os}/{arch}")),
    }
}

/// Download (first run) and start the provider. `SPOTTY_NO_POT=1` opts out.
pub(crate) fn ensure_and_start() -> Result<(), String> {
    if std::env::var_os("SPOTTY_NO_POT").is_some() {
        return Ok(());
    }
    let (asset, sum, exe) = server_asset()?;
    let dir = bin_dir();
    let quiet = |_: String| {};
    let plugin = dir.join("yt-dlp-plugins/bgutil-ytdlp-pot-provider.zip");
    if !plugin.exists() {
        std::fs::create_dir_all(plugin.parent().unwrap()).map_err(|e| e.to_string())?;
        download(PLUGIN.0, &plugin, PLUGIN.1, "token plugin", &quiet)?;
    }
    let server = dir.join(format!("bgutil-pot{exe}"));
    if !server.exists() {
        download(
            &format!("{SERVER}/{asset}"),
            &server,
            sum,
            "token provider",
            &quiet,
        )?;
        crate::setup::make_executable(&server)?;
    }
    if std::net::TcpStream::connect_timeout(&ADDR.parse().unwrap(), Duration::from_millis(300))
        .is_ok()
    {
        return Ok(()); // already running
    }
    let mut c = Command::new(&server);
    c.arg("server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::util::quiet(&mut c);
    *CHILD.lock().unwrap() = Some(c.spawn().map_err(|e| e.to_string())?);
    Ok(())
}

pub(crate) fn stop() {
    if let Some(mut c) = CHILD.lock().unwrap().take() {
        let _ = c.kill();
    }
}
