//! Bandwidth measurement shared by every shell: an idle-latency probe, a
//! download phase and an upload phase against Cloudflare's keyless speed-test
//! endpoints.
//!
//! Transport is the system `curl`, the same choice `look-answers` made for its
//! HTTP helper, so this crate needs no async runtime and no TLS dependency.
//! `curl` already measures each transfer for us, so a phase is "spawn N of them
//! and sum what they report".

use serde::Serialize;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

mod endpoint {
    /// Keyless and account-free: `__down` streams `bytes` of payload, `__up`
    /// accepts a POST body of any size.
    pub const DOWNLOAD: &str = "https://speed.cloudflare.com/__down?bytes=";
    pub const UPLOAD: &str = "https://speed.cloudflare.com/__up";
    /// Zero-byte download, so the timing covers the round trip and nothing else.
    pub const LATENCY: &str = "https://speed.cloudflare.com/__down?bytes=0";
    /// Keyless lookup of who carries the traffic and roughly from where, the
    /// same service the weather tile geocodes against.
    pub const ORIGIN: &str = "https://ipwho.is/";

    /// Where the download phase goes when Cloudflare refuses. These are plain
    /// files published for speed testing, ranged to the size we want, so they
    /// carry no per-connection budget to trip. They are fixed locations, hence
    /// the round-trip probe that picks the closest one.
    pub const MIRRORS: &[(&str, &str)] = &[
        (
            "Linode Tokyo",
            "https://speedtest.tokyo2.linode.com/100MB-tokyo2.bin",
        ),
        (
            "Linode Newark",
            "https://speedtest.newark.linode.com/100MB-newark.bin",
        ),
        (
            "Linode Frankfurt",
            "https://speedtest.frankfurt.linode.com/100MB-frankfurt.bin",
        ),
        (
            "Hetzner Nuremberg",
            "https://nbg1-speed.hetzner.com/100MB.bin",
        ),
    ];
}

const CLOUDFLARE: &str = "Cloudflare";
/// How long a mirror gets to answer the round-trip probe that ranks it.
const MIRROR_PROBE_TIMEOUT_SECS: u32 = 4;
/// Only the two closest mirrors are actually tried. Walking the whole list on a
/// bad connection would cost a full download timeout each, pushing a run far
/// past the wait anyone expects from it.
const MAX_MIRROR_ATTEMPTS: usize = 2;

/// A single TCP stream leaves a fast link idle, so each phase runs several and
/// sums what they report.
const STREAMS: usize = 4;
/// Kept modest on purpose: the endpoint rate-limits by volume, and a run that
/// trips that limit measures nothing at all. 4 x 10 MB is enough to fill a fast
/// link for long enough to read it.
const DOWNLOAD_BYTES_PER_STREAM: usize = 10_000_000;
const UPLOAD_BYTES_PER_STREAM: usize = 5_000_000;
/// Caps each phase. A slow link measures whatever it managed inside the window
/// rather than holding the panel open.
const DOWNLOAD_TIMEOUT_SECS: u32 = 8;
const UPLOAD_TIMEOUT_SECS: u32 = 8;
const LATENCY_TIMEOUT_SECS: u32 = 5;
const LATENCY_SAMPLES: usize = 3;
const ORIGIN_TIMEOUT_SECS: u32 = 5;
/// Ask for the IPv4 address first. It is the one people recognise and quote,
/// and it fits a line; a dual-stack machine otherwise answers with its IPv6.
const IPV4_ONLY: &str = "-4";

const TOO_MANY_REQUESTS: u32 = 429;
const OK_STATUS: std::ops::Range<u32> = 200..300;
/// Filler byte for the upload body. Cloudflare does not compress request
/// bodies, so the content does not affect the measurement.
const UPLOAD_FILLER: u8 = b'x';

const BITS_PER_BYTE: f64 = 8.0;
const MILLIS_PER_SEC: f64 = 1_000.0;
const KILOBIT: f64 = 1_000.0;
const MEGABIT: f64 = 1_000_000.0;
const GIGABIT: f64 = 1_000_000_000.0;
/// Shown wherever a phase produced nothing.
const UNAVAILABLE: &str = "n/a";

/// Plain-language read of a rate, so a number nobody can place ("240 Mbps")
/// lands as something they can ("busy household"). Each entry is the floor it
/// applies from, in Mbps.
const DOWNLOAD_VERDICTS: &[(f64, &str)] = &[
    (0.0, "a single email"),
    (1.0, "dial-up, roughly"),
    (5.0, "SD video"),
    (10.0, "one HD stream"),
    (25.0, "4K, one screen"),
    (50.0, "busy household"),
    (100.0, "fast broadband"),
    (300.0, "gigabit-ish"),
    (700.0, "fibre, showing off"),
];

/// Where latency stops being fine and starts being a problem. Kept beside the
/// words below so a shell colours the reading on the same boundaries it reads
/// it by, rather than inventing a second scale.
const LATENCY_WARN_MS: f64 = 45.0;
const LATENCY_BAD_MS: f64 = 150.0;

/// The same for latency, in milliseconds.
const LATENCY_VERDICTS: &[(f64, &str)] = &[
    (0.0, "instant"),
    (20.0, "excellent"),
    (45.0, "fine"),
    (80.0, "noticeable"),
    (150.0, "annoying"),
    (300.0, "painful"),
    (500.0, "satellite, probably"),
];

#[cfg(target_os = "windows")]
const NULL_SINK: &str = "NUL";
#[cfg(not(target_os = "windows"))]
const NULL_SINK: &str = "/dev/null";

// Suppress the console window when curl spawns from a GUI shell.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One completed measurement: raw rates alongside the text every shell prints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedReading {
    pub download_bits_per_second: f64,
    pub upload_bits_per_second: f64,
    /// Idle round trip to the endpoint. `None` when every probe failed.
    pub latency_ms: Option<f64>,
    pub download_display: String,
    pub upload_display: String,
    pub latency_display: String,
    /// Plain-language reads of the two numbers above.
    pub download_verdict: String,
    pub latency_verdict: String,
    /// How the latency should read: `good`, `warn` or `bad`.
    pub latency_level: &'static str,
    /// Which server the download phase measured against, when one answered.
    pub download_source: Option<String>,
    /// The address the far end sees, the carrier behind it, and roughly where
    /// it lands. All three are absent when the lookup fails, which never fails
    /// the measurement itself.
    pub public_ip: Option<String>,
    pub provider: Option<String>,
    pub location: Option<String>,
    /// Seconds since the Unix epoch, so a shell can age the reading it cached.
    pub measured_at_unix: i64,
}

/// Why a run produced no reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedError {
    CurlMissing,
    NoConnection,
    RateLimited,
}

impl SpeedError {
    pub fn message(&self) -> &'static str {
        match self {
            SpeedError::CurlMissing => "curl is not available on this system",
            SpeedError::NoConnection => "Could not reach the speed test server",
            SpeedError::RateLimited => {
                "Speed test server is rate limiting this connection, try again shortly"
            }
        }
    }
}

/// Runs every phase and returns the reading. Blocks: about 15 seconds when the
/// primary answers, and up to roughly 50 when every phase has to time out and
/// fall back. Callers must drive it off their UI thread. There is no cancel;
/// per-phase timeouts and `MAX_MIRROR_ATTEMPTS` are what bound the wait.
pub fn run() -> Result<SpeedReading, SpeedError> {
    if !curl_available() {
        return Err(SpeedError::CurlMissing);
    }

    // Metadata, and a slow or unreachable lookup would otherwise hold up every
    // measurement behind it. Its ~1 KB is noise against the transfer phases.
    let (origin, latency_ms, download, upload) = std::thread::scope(|scope| {
        let origin = scope.spawn(fetch_origin);
        let latency_ms = measure_latency();
        let download = measure_download();
        let upload = measure_upload();
        (
            origin.join().unwrap_or_default(),
            latency_ms,
            download,
            upload,
        )
    });
    // A phase that measured nothing reports no rate rather than a zero one,
    // which would read as a slow link rather than a refused one. The run only
    // fails when neither direction got through.
    let (download_bits_per_second, download_source) = match download {
        Ok(measured) => (measured.bits_per_second, Some(measured.source.to_string())),
        Err(error) => {
            if upload.is_err() {
                return Err(error);
            }
            (0.0, None)
        }
    };
    let upload_bits_per_second = upload.unwrap_or_default();

    Ok(SpeedReading {
        download_bits_per_second,
        upload_bits_per_second,
        latency_ms,
        download_display: format_rate(download_bits_per_second),
        upload_display: format_rate(upload_bits_per_second),
        latency_display: format_latency(latency_ms),
        download_verdict: download_verdict(download_bits_per_second).to_string(),
        latency_verdict: latency_verdict(latency_ms).to_string(),
        latency_level: latency_level(latency_ms),
        download_source,
        public_ip: origin.public_ip,
        provider: origin.provider,
        location: origin.location,
        measured_at_unix: now_unix(),
    })
}

/// `run` as JSON, for the FFI bridge and the Tauri command: either
/// `{"ok":true,"reading":{...}}` or `{"ok":false,"error":"..."}`.
pub fn run_json() -> String {
    match run() {
        Ok(reading) => serde_json::json!({ "ok": true, "reading": reading }).to_string(),
        Err(error) => serde_json::json!({ "ok": false, "error": error.message() }).to_string(),
    }
}

/// Bits per second as display text, e.g. `92.4 Mbps`.
fn format_rate(bits_per_second: f64) -> String {
    if !bits_per_second.is_finite() || bits_per_second <= 0.0 {
        return UNAVAILABLE.to_string();
    }
    if bits_per_second >= GIGABIT {
        return format!("{:.2} Gbps", bits_per_second / GIGABIT);
    }
    if bits_per_second >= MEGABIT {
        return format!("{:.1} Mbps", bits_per_second / MEGABIT);
    }
    format!("{:.0} Kbps", bits_per_second / KILOBIT)
}

/// Milliseconds as display text, e.g. `26 ms`.
fn format_latency(latency_ms: Option<f64>) -> String {
    match latency_ms {
        Some(value) if value.is_finite() && value > 0.0 => format!("{value:.0} ms"),
        _ => UNAVAILABLE.to_string(),
    }
}

/// How a download rate reads to someone who doesn't think in Mbps.
fn download_verdict(bits_per_second: f64) -> &'static str {
    verdict(DOWNLOAD_VERDICTS, bits_per_second / MEGABIT)
}

/// The same for latency. `None` has nothing to judge.
fn latency_verdict(latency_ms: Option<f64>) -> &'static str {
    match latency_ms {
        Some(value) => verdict(LATENCY_VERDICTS, value),
        None => UNAVAILABLE,
    }
}

fn latency_level(latency_ms: Option<f64>) -> &'static str {
    match latency_ms {
        Some(value) if value >= LATENCY_BAD_MS => "bad",
        Some(value) if value >= LATENCY_WARN_MS => "warn",
        Some(_) => "good",
        None => "unknown",
    }
}

/// The last entry whose floor the value has reached.
fn verdict(scale: &[(f64, &'static str)], value: f64) -> &'static str {
    scale
        .iter()
        .rev()
        .find(|(floor, _)| value >= *floor)
        .map(|(_, label)| *label)
        .unwrap_or(UNAVAILABLE)
}

#[derive(Debug, Default, Clone, PartialEq)]
struct Origin {
    public_ip: Option<String>,
    provider: Option<String>,
    location: Option<String>,
}

/// An IPv6-only network has nothing to say over IPv4, so that attempt is
/// allowed to fail and the retry takes whatever the system prefers.
fn fetch_origin() -> Origin {
    origin_lookup(Some(IPV4_ONLY))
        .or_else(|| origin_lookup(None))
        .unwrap_or_default()
}

fn origin_lookup(family: Option<&str>) -> Option<Origin> {
    let timeout = ORIGIN_TIMEOUT_SECS.to_string();
    let mut command = curl_command();
    if let Some(family) = family {
        command.arg(family);
    }
    command.args(["-s", "--max-time", &timeout, endpoint::ORIGIN]);

    let output = command.output().ok()?;
    let body = String::from_utf8(output.stdout).ok()?;
    let payload = serde_json::from_str::<serde_json::Value>(&body).ok()?;
    let public_ip = text(&payload["ip"])?;

    Some(Origin {
        public_ip: Some(public_ip),
        provider: text(&payload["connection"]["isp"])
            .or_else(|| text(&payload["connection"]["org"])),
        location: compose_location(text(&payload["city"]), text(&payload["country_code"])),
    })
}

/// "Tokyo, JP", or whichever half of it came back.
fn compose_location(city: Option<String>, country_code: Option<String>) -> Option<String> {
    match (city, country_code) {
        (Some(city), Some(code)) => Some(format!("{city}, {code}")),
        (Some(city), None) => Some(city),
        (None, Some(code)) => Some(code),
        (None, None) => None,
    }
}

fn text(value: &serde_json::Value) -> Option<String> {
    let text = value.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Lane {
    Rate(f64),
    RateLimited,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
struct Measured {
    bits_per_second: f64,
    source: &'static str,
}

/// Cloudflare first: it sizes the payload on request and is usually the closest
/// of the lot. Its budget is per-connection though, so a few runs in a row can
/// be refused, and then a mirror answers instead. No cooldown is kept: a refusal
/// comes back instantly, so retrying it first costs nothing and it recovers on
/// its own.
fn measure_download() -> Result<Measured, SpeedError> {
    let url = format!("{}{}", endpoint::DOWNLOAD, DOWNLOAD_BYTES_PER_STREAM);
    match phase(|_| download_stream(&url, None)) {
        Ok(bits_per_second) => Ok(Measured {
            bits_per_second,
            source: CLOUDFLARE,
        }),
        Err(cloudflare_error) => mirror_download().ok_or(cloudflare_error),
    }
}

/// The upload phase has no comparable free sink to fall back to, so a refusal
/// here leaves the direction unmeasured rather than failing the run. One buffer
/// is shared by every lane: it is write-only filler, identical across them.
fn measure_upload() -> Result<f64, SpeedError> {
    let payload = vec![UPLOAD_FILLER; UPLOAD_BYTES_PER_STREAM];
    phase(|_| upload_stream(&payload))
}

/// Ranks the mirrors by round trip and measures against the nearest that
/// answers, so a fixed location on the other side of the world doesn't get
/// reported as the link's speed.
fn mirror_download() -> Option<Measured> {
    let mut ranked: Vec<(f64, &'static str, &'static str)> = std::thread::scope(|scope| {
        let probes: Vec<_> = endpoint::MIRRORS
            .iter()
            .map(|(name, url)| {
                scope.spawn(move || probe_round_trip(url).map(|rtt| (rtt, *name, *url)))
            })
            .collect();
        probes
            .into_iter()
            .filter_map(|probe| probe.join().ok().flatten())
            .collect()
    });
    ranked.sort_by(|left, right| left.0.total_cmp(&right.0));

    for (_, name, url) in ranked.into_iter().take(MAX_MIRROR_ATTEMPTS) {
        // Each lane takes its own slice of the file, so the phase measures four
        // real transfers rather than four requests for the same bytes.
        if let Ok(bits_per_second) = phase(|lane| download_stream(url, Some(&lane_range(lane)))) {
            return Some(Measured {
                bits_per_second,
                source: name,
            });
        }
    }
    None
}

fn lane_range(lane: usize) -> String {
    let start = lane * DOWNLOAD_BYTES_PER_STREAM;
    format!("{}-{}", start, start + DOWNLOAD_BYTES_PER_STREAM - 1)
}

/// Time to first byte of a token-sized request, as the mirror's distance. A
/// refused or unreachable mirror has no distance, and drops out of the ranking.
fn probe_round_trip(url: &str) -> Option<f64> {
    let output = measuring_curl(
        MIRROR_PROBE_TIMEOUT_SECS,
        "%{http_code} %{time_starttransfer}",
    )
    .args(["-r", "0-1"])
    .arg(url)
    .output()
    .ok()?;

    match parse_lane(&output.stdout) {
        Lane::Rate(seconds) => Some(seconds),
        _ => None,
    }
}

/// A rate-limited lane is reported as such even if others merely failed: it is
/// the reason worth acting on.
fn combine(lanes: Vec<Lane>) -> Result<f64, SpeedError> {
    let total: f64 = lanes
        .iter()
        .filter_map(|lane| match lane {
            Lane::Rate(rate) => Some(rate),
            _ => None,
        })
        .sum();

    if total > 0.0 {
        return Ok(total * BITS_PER_BYTE);
    }
    if lanes.contains(&Lane::RateLimited) {
        return Err(SpeedError::RateLimited);
    }
    Err(SpeedError::NoConnection)
}

/// Runs every lane at once and totals what they measured.
fn phase<F>(stream: F) -> Result<f64, SpeedError>
where
    F: Fn(usize) -> Lane + Sync,
{
    combine(run_lanes(stream))
}

fn run_lanes<F>(stream: F) -> Vec<Lane>
where
    F: Fn(usize) -> Lane + Sync,
{
    std::thread::scope(|scope| {
        let lanes: Vec<_> = (0..STREAMS)
            .map(|index| {
                let stream = &stream;
                scope.spawn(move || stream(index))
            })
            .collect();
        lanes
            .into_iter()
            .map(|lane| lane.join().unwrap_or(Lane::Failed))
            .collect()
    })
}

/// `range` bounds a mirror's fixed-size file to the slice we want; Cloudflare
/// sizes its own payload and passes `None`.
fn download_stream(url: &str, range: Option<&str>) -> Lane {
    let mut command = measuring_curl(DOWNLOAD_TIMEOUT_SECS, "%{http_code} %{speed_download}");
    if let Some(range) = range {
        command.args(["-r", range]);
    }
    command.arg(url);
    command
        .output()
        .map(|output| parse_lane(&output.stdout))
        .unwrap_or(Lane::Failed)
}

fn upload_stream(payload: &[u8]) -> Lane {
    let mut command = measuring_curl(UPLOAD_TIMEOUT_SECS, "%{http_code} %{speed_upload}");
    command
        .args(["-X", "POST", "--data-binary", "@-", endpoint::UPLOAD])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let Ok(mut child) = command.spawn() else {
        return Lane::Failed;
    };

    if let Some(mut stdin) = child.stdin.take() {
        // A timeout closes curl's end mid-write; that broken pipe is the
        // timeout doing its job, and curl still reports what it sent.
        let _ = stdin.write_all(payload);
    }

    child
        .wait_with_output()
        .map(|output| parse_lane(&output.stdout))
        .unwrap_or(Lane::Failed)
}

/// Best of several probes, since any single one can catch a scheduling hiccup.
fn measure_latency() -> Option<f64> {
    (0..LATENCY_SAMPLES)
        .filter_map(|_| {
            measuring_curl(LATENCY_TIMEOUT_SECS, "%{time_namelookup} %{time_connect}")
                .arg(endpoint::LATENCY)
                .output()
                .ok()
                .and_then(|output| parse_round_trip(&output.stdout))
        })
        .min_by(f64::total_cmp)
}

fn curl_available() -> bool {
    let mut command = curl_command();
    command.arg("--version");
    matches!(command.output(), Ok(output) if output.status.success())
}

/// Every measuring call is the same curl invocation bar one flag: quiet, body
/// discarded, capped, and reporting the numbers we read.
fn measuring_curl(timeout_secs: u32, write_out: &str) -> Command {
    let mut command = curl_command();
    command.args([
        "-s",
        "-o",
        NULL_SINK,
        "--max-time",
        &timeout_secs.to_string(),
        "-w",
        write_out,
    ]);
    command
}

fn curl_command() -> Command {
    // Only the Linux and Windows arms below mutate it.
    #[allow(unused_mut)]
    let mut command = Command::new("curl");
    // The AppImage points LD_LIBRARY_PATH at bundled Ubuntu libs, against which
    // the system curl fails to resolve libcurl (see `look-answers`).
    #[cfg(target_os = "linux")]
    command.env_remove("LD_LIBRARY_PATH");
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Reads `http_code speed` from `--write-out`. The status has to be checked:
/// an error page is a transfer as far as the byte counter is concerned, and a
/// rate-limited run would otherwise report a very slow link. Most curl builds
/// print in the C locale; a few use a decimal comma.
fn parse_lane(raw: &[u8]) -> Lane {
    let text = String::from_utf8_lossy(raw).replace(',', ".");
    let mut fields = text.split_whitespace();

    let Some(status) = fields.next().and_then(|field| field.parse::<u32>().ok()) else {
        return Lane::Failed;
    };
    if status == TOO_MANY_REQUESTS {
        return Lane::RateLimited;
    }
    if !OK_STATUS.contains(&status) {
        return Lane::Failed;
    }

    match fields.next().and_then(|field| field.parse::<f64>().ok()) {
        Some(rate) if rate.is_finite() && rate > 0.0 => Lane::Rate(rate),
        _ => Lane::Failed,
    }
}

/// `time_namelookup time_connect`, in seconds, as the round trip in ms. curl's
/// timers are cumulative from the start of the request, so the lookup has to
/// come off the connect or a cold DNS resolve reads as latency.
fn parse_round_trip(raw: &[u8]) -> Option<f64> {
    let text = String::from_utf8_lossy(raw).replace(',', ".");
    let mut fields = text.split_whitespace();
    let lookup: f64 = fields.next()?.parse().ok()?;
    let connect: f64 = fields.next()?.parse().ok()?;

    let round_trip = (connect - lookup) * MILLIS_PER_SEC;
    if !round_trip.is_finite() || round_trip <= 0.0 {
        return None;
    }
    Some(round_trip)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_each_magnitude() {
        assert_eq!(format_rate(2_500_000_000.0), "2.50 Gbps");
        assert_eq!(format_rate(92_400_000.0), "92.4 Mbps");
        assert_eq!(format_rate(780_000.0), "780 Kbps");
    }

    #[test]
    fn formats_a_missing_rate_as_unavailable() {
        assert_eq!(format_rate(0.0), UNAVAILABLE);
        assert_eq!(format_rate(-1.0), UNAVAILABLE);
        assert_eq!(format_rate(f64::NAN), UNAVAILABLE);
    }

    #[test]
    fn reads_a_rate_in_plain_language() {
        assert_eq!(download_verdict(60.0 * MEGABIT), "busy household");
        assert_eq!(download_verdict(240.0 * MEGABIT), "fast broadband");
        assert_eq!(download_verdict(940.0 * MEGABIT), "fibre, showing off");
        assert_eq!(download_verdict(0.05 * MEGABIT), "a single email");
    }

    #[test]
    fn levels_latency_on_the_same_boundaries_it_reads_it_by() {
        assert_eq!(latency_level(Some(16.0)), "good");
        assert_eq!(latency_level(Some(LATENCY_WARN_MS)), "warn");
        assert_eq!(latency_level(Some(LATENCY_BAD_MS)), "bad");
        assert_eq!(latency_level(None), "unknown");
    }

    #[test]
    fn reads_latency_in_plain_language() {
        assert_eq!(latency_verdict(Some(3.0)), "instant");
        assert_eq!(latency_verdict(Some(16.0)), "instant");
        assert_eq!(latency_verdict(Some(38.0)), "excellent");
        assert_eq!(latency_verdict(Some(620.0)), "satellite, probably");
        assert_eq!(latency_verdict(None), UNAVAILABLE);
    }

    #[test]
    fn formats_latency_only_when_measured() {
        assert_eq!(format_latency(Some(26.4)), "26 ms");
        assert_eq!(format_latency(None), UNAVAILABLE);
    }

    #[test]
    fn gives_each_lane_its_own_slice_of_a_mirror_file() {
        assert_eq!(
            lane_range(0),
            format!("0-{}", DOWNLOAD_BYTES_PER_STREAM - 1)
        );
        assert_eq!(
            lane_range(2),
            format!(
                "{}-{}",
                2 * DOWNLOAD_BYTES_PER_STREAM,
                3 * DOWNLOAD_BYTES_PER_STREAM - 1
            )
        );
    }

    #[test]
    fn parses_a_lane_in_either_locale() {
        assert_eq!(parse_lane(b"200 11500000.000000"), Lane::Rate(11_500_000.0));
        assert_eq!(parse_lane(b"200 11500000,000000"), Lane::Rate(11_500_000.0));
    }

    #[test]
    fn treats_a_rate_limited_lane_as_refused_not_slow() {
        // The 429 body is one byte, which the byte counter happily calls a
        // transfer: 21 B/s reads as "0 Kbps" unless the status is checked.
        assert_eq!(parse_lane(b"429 21.000000"), Lane::RateLimited);
    }

    #[test]
    fn a_phase_sums_what_measured_and_names_what_did_not() {
        assert_eq!(
            combine(vec![Lane::Rate(1.0), Lane::Failed, Lane::Rate(2.0)]),
            Ok(3.0 * BITS_PER_BYTE)
        );
        assert_eq!(
            combine(vec![Lane::Failed, Lane::RateLimited]),
            Err(SpeedError::RateLimited)
        );
        assert_eq!(
            combine(vec![Lane::Failed, Lane::Failed]),
            Err(SpeedError::NoConnection)
        );
    }

    #[test]
    fn composes_whichever_half_of_the_location_came_back() {
        let city = || Some("Tokyo".to_string());
        let code = || Some("JP".to_string());
        assert_eq!(
            compose_location(city(), code()),
            Some("Tokyo, JP".to_string())
        );
        assert_eq!(compose_location(city(), None), Some("Tokyo".to_string()));
        assert_eq!(compose_location(None, code()), Some("JP".to_string()));
        assert_eq!(compose_location(None, None), None);
    }

    #[test]
    fn reads_only_non_empty_strings() {
        assert_eq!(
            text(&serde_json::json!("  Tokyo  ")),
            Some("Tokyo".to_string())
        );
        assert_eq!(text(&serde_json::json!("   ")), None);
        assert_eq!(text(&serde_json::json!(null)), None);
        assert_eq!(text(&serde_json::json!(42)), None);
    }

    #[test]
    fn takes_the_round_trip_without_the_dns_lookup() {
        // 4ms resolving, connect complete at 20ms: the round trip is the 16ms
        // between them, not the 20 curl reports.
        let round_trip = parse_round_trip(b"0.004000 0.020000").expect("parsed");
        assert!((round_trip - 16.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_a_probe_that_never_connected() {
        assert_eq!(parse_round_trip(b"0.004000 0.000000"), None);
        assert_eq!(parse_round_trip(b"0.004000"), None);
        assert_eq!(parse_round_trip(b"curl: (6) Could not resolve host"), None);
    }

    #[test]
    fn rejects_a_lane_that_measured_nothing() {
        assert_eq!(parse_lane(b"200 0.000000"), Lane::Failed);
        assert_eq!(parse_lane(b"500 900.0"), Lane::Failed);
        assert_eq!(parse_lane(b""), Lane::Failed);
        assert_eq!(
            parse_lane(b"curl: (6) Could not resolve host"),
            Lane::Failed
        );
    }
}
