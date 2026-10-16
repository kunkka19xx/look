//! The `/speed` commands: the shared `look-netspeed` measurement macOS reaches
//! through the FFI bridge, plus the LAN address the panel shows beside it.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;

pub use look_netspeed::SpeedReading;
use serde::Deserialize;

use crate::crash::state_dir;

/// The last reading, under the state dir, so the panel opens with a number
/// instead of a blank. The webview keeps the same thing in localStorage.
const LAST_READING_FILE: &str = "speedtest-last-reading.json";

/// Where the route lookup pretends to be headed. Nothing is sent to it.
const ROUTE_PROBE: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 80);

/// Blocks for many seconds, so the shell runs it on its blocking pool.
pub fn speed_test() -> Result<SpeedReading, String> {
    look_netspeed::run().map_err(|error| error.message().to_string())
}

/// This machine's address on the local network. `connect` on a UDP socket only
/// asks the routing table which interface would carry the traffic, so no packet
/// leaves and nothing blocks. `None` when only loopback is up.
pub fn local_ipv4() -> Option<String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(ROUTE_PROBE).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(address) if !address.is_loopback() && !address.is_unspecified() => {
            Some(address.to_string())
        }
        _ => None,
    }
}

fn last_reading_path() -> Option<PathBuf> {
    Some(state_dir()?.join(LAST_READING_FILE))
}

/// The reading as written by `remember`. Core types the level as a static
/// str, so it comes back through text.
#[derive(Deserialize)]
struct StoredReading {
    download_bits_per_second: f64,
    upload_bits_per_second: f64,
    latency_ms: Option<f64>,
    download_display: String,
    upload_display: String,
    latency_display: String,
    download_verdict: String,
    latency_verdict: String,
    latency_level: String,
    download_source: Option<String>,
    public_ip: Option<String>,
    provider: Option<String>,
    location: Option<String>,
    measured_at_unix: i64,
}

/// The words core bands latency into, back to the static strs it uses.
fn latency_level(word: &str) -> &'static str {
    match word {
        "good" => "good",
        "warn" => "warn",
        "bad" => "bad",
        _ => "unknown",
    }
}

/// What the last run left, if anything is on disk.
pub fn last_reading() -> Option<SpeedReading> {
    let text = std::fs::read_to_string(last_reading_path()?).ok()?;
    let stored: StoredReading = serde_json::from_str(&text).ok()?;
    Some(SpeedReading {
        download_bits_per_second: stored.download_bits_per_second,
        upload_bits_per_second: stored.upload_bits_per_second,
        latency_ms: stored.latency_ms,
        download_display: stored.download_display,
        upload_display: stored.upload_display,
        latency_display: stored.latency_display,
        download_verdict: stored.download_verdict,
        latency_verdict: stored.latency_verdict,
        latency_level: latency_level(&stored.latency_level),
        download_source: stored.download_source,
        public_ip: stored.public_ip,
        provider: stored.provider,
        location: stored.location,
        measured_at_unix: stored.measured_at_unix,
    })
}

/// Keep the reading for the next launch, less the public address: the panel
/// masks it on screen, and a file keeping it indefinitely is a stronger
/// retention than that implies. A fresh run puts it back.
pub fn remember(reading: &SpeedReading) {
    let Some(path) = last_reading_path() else {
        return;
    };
    let stored = SpeedReading {
        public_ip: None,
        ..reading.clone()
    };
    let Ok(text) = serde_json::to_string(&stored) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = std::fs::write(&path, text) {
        eprintln!("[speed] remembering the reading: {err}");
    }
}
