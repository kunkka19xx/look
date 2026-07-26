//! Weather for the launchpad tile. Mirrors the macOS `WeatherService`: an
//! approximate location from a keyless IP lookup (city-level, no permission
//! prompt) plus current conditions from Open-Meteo (keyless, free).
//!
//! Both fetches go through the shared `look-answers` curl transport (no async
//! runtime, and it scrubs the AppImage's `LD_LIBRARY_PATH` so the system curl
//! resolves its own libs). Readings are cached in-process so reopening the
//! launcher shows the last value instantly and the network is hit at most once
//! per refresh interval; the launcher is resident, so the cache survives every
//! window summon. On a network failure the last reading is returned, so the
//! tile degrades to stale rather than blank.

use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const IP_GEOLOCATION_URL: &str = "https://ipwho.is/";
const OPEN_METEO_URL: &str = "https://api.open-meteo.com/v1/forecast";

/// Skip the network when the cached reading is younger than this.
const WEATHER_MAX_AGE: Duration = Duration::from_secs(30 * 60);
/// Location changes rarely; reuse the cached coordinates for this long.
const LOCATION_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// Cap each request so a hung network never stalls the refresh.
const REQUEST_TIMEOUT_SECS: u32 = 8;

static WEATHER_CACHE: Mutex<Option<CachedWeather>> = Mutex::new(None);
static LOCATION_CACHE: Mutex<Option<Coordinates>> = Mutex::new(None);

/// A resolved reading ready for the launchpad tile. `symbol` is a condition key
/// the frontend maps to an icon; the temperatures already carry the degree sign.
#[derive(Debug, Clone, Serialize)]
pub struct WeatherSnapshot {
    pub temperature: String,
    pub symbol: String,
    pub condition: String,
    pub high: String,
    pub low: String,
    /// Today's chance of precipitation as a percent (e.g. "60%"), or `None` when
    /// the source didn't report it.
    pub rain_chance: Option<String>,
}

#[derive(Clone)]
struct Coordinates {
    latitude: f64,
    longitude: f64,
    fetched_at: Instant,
}

#[derive(Clone)]
struct CachedWeather {
    temperature: f64,
    code: i64,
    high: f64,
    low: f64,
    rain_chance: Option<i64>,
    is_fahrenheit: bool,
    fetched_at: Instant,
}

/// Live weather for the launchpad tile, or `None` when it can't be resolved
/// (offline first run, no location). Runs the blocking fetch off the UI thread.
#[tauri::command]
pub async fn weather_current() -> Option<WeatherSnapshot> {
    tauri::async_runtime::spawn_blocking(current)
        .await
        .ok()
        .flatten()
}

/// Current weather, from cache when still fresh and otherwise the network.
/// Returns the last cached reading (or `None`) when the network is unavailable.
/// Blocks on curl, so callers run it off the UI thread.
fn current() -> Option<WeatherSnapshot> {
    if let Some(cached) = usable_cached()
        && cached.fetched_at.elapsed() < WEATHER_MAX_AGE
    {
        return Some(snapshot(&cached));
    }

    match coordinates().and_then(|c| fetch_weather(&c)) {
        Some(reading) => {
            *WEATHER_CACHE.lock().unwrap() = Some(reading.clone());
            Some(snapshot(&reading))
        }
        None => usable_cached().as_ref().map(snapshot),
    }
}

/// The cached reading, but only when it was captured in the unit the current
/// locale wants. After a region change the old value is in the wrong unit, so
/// it is discarded to force a refetch rather than shown as a bare degree.
fn usable_cached() -> Option<CachedWeather> {
    WEATHER_CACHE
        .lock()
        .unwrap()
        .clone()
        .filter(|c| c.is_fahrenheit == uses_fahrenheit())
}

// --- Location ---------------------------------------------------------------

fn coordinates() -> Option<Coordinates> {
    if let Some(cached) = LOCATION_CACHE.lock().unwrap().clone()
        && cached.fetched_at.elapsed() < LOCATION_MAX_AGE
    {
        return Some(cached);
    }
    let fresh = fetch_coordinates()?;
    *LOCATION_CACHE.lock().unwrap() = Some(fresh.clone());
    Some(fresh)
}

fn fetch_coordinates() -> Option<Coordinates> {
    let body = look_answers::http::get_json(IP_GEOLOCATION_URL, REQUEST_TIMEOUT_SECS)?;
    // ipwho.is reports `success: false` (and omits coordinates) on failure.
    if body.get("success")?.as_bool()? {
        Some(Coordinates {
            latitude: body.get("latitude")?.as_f64()?,
            longitude: body.get("longitude")?.as_f64()?,
            fetched_at: Instant::now(),
        })
    } else {
        None
    }
}

// --- Weather ----------------------------------------------------------------

fn fetch_weather(coordinates: &Coordinates) -> Option<CachedWeather> {
    let fahrenheit = uses_fahrenheit();
    let url = format!(
        "{OPEN_METEO_URL}?latitude={}&longitude={}&current=temperature_2m,weather_code\
         &daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max\
         &temperature_unit={}&timezone=auto&forecast_days=1",
        coordinates.latitude,
        coordinates.longitude,
        if fahrenheit { "fahrenheit" } else { "celsius" },
    );
    let body = look_answers::http::get_json(&url, REQUEST_TIMEOUT_SECS)?;
    let current = body.get("current")?;
    let daily = body.get("daily")?;
    Some(CachedWeather {
        temperature: current.get("temperature_2m")?.as_f64()?,
        code: current.get("weather_code")?.as_i64().unwrap_or(0),
        high: first_f64(daily, "temperature_2m_max")?,
        low: first_f64(daily, "temperature_2m_min")?,
        // A single slot can be null even when the array is present.
        rain_chance: daily
            .get("precipitation_probability_max")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_i64()),
        is_fahrenheit: fahrenheit,
        fetched_at: Instant::now(),
    })
}

/// First element of a single-day forecast array (the request asks for one day).
fn first_f64(daily: &serde_json::Value, key: &str) -> Option<f64> {
    daily.get(key)?.as_array()?.first()?.as_f64()
}

/// Fahrenheit for US-style locales, Celsius everywhere else. Mirrors the macOS
/// `Locale.measurementSystem == .us` check, read from the standard locale env.
#[cfg(not(target_os = "windows"))]
fn uses_fahrenheit() -> bool {
    ["LC_MEASUREMENT", "LC_ALL", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
        .is_some_and(|locale| locale.contains("US"))
}

/// Fahrenheit only in the US, matching the macOS `.us` check. Reads the home
/// region (not the display-format locale, which is en-US even for someone in a
/// metric country and would wrongly force Fahrenheit).
#[cfg(target_os = "windows")]
fn uses_fahrenheit() -> bool {
    use windows::Win32::Globalization::GetUserDefaultGeoName;
    // ISO 3166 two-letter region, e.g. "US", "JP". len counts the trailing NUL.
    let mut buf = [0u16; 16];
    let len = unsafe { GetUserDefaultGeoName(&mut buf) };
    if len <= 0 {
        return false;
    }
    let chars = (len as usize).saturating_sub(1);
    String::from_utf16_lossy(&buf[..chars])
        .trim()
        .eq_ignore_ascii_case("US")
}

// --- Presentation -----------------------------------------------------------

fn snapshot(reading: &CachedWeather) -> WeatherSnapshot {
    let (symbol, condition) = condition(reading.code);
    WeatherSnapshot {
        temperature: format_temp(reading.temperature),
        symbol: symbol.to_string(),
        condition: condition.to_string(),
        high: format_temp(reading.high),
        low: format_temp(reading.low),
        rain_chance: reading.rain_chance.map(|c| format!("{c}%")),
    }
}

fn format_temp(value: f64) -> String {
    format!("{}\u{00b0}", value.round() as i64)
}

/// Maps a WMO weather-interpretation code to a frontend icon key and a short
/// label. Codes are grouped per the Open-Meteo documentation.
fn condition(code: i64) -> (&'static str, &'static str) {
    match code {
        0 => ("clear", "Clear"),
        1 | 2 => ("partly", "Partly Cloudy"),
        3 => ("cloudy", "Cloudy"),
        45 | 48 => ("fog", "Fog"),
        51 | 53 | 55 | 56 | 57 => ("drizzle", "Drizzle"),
        61 | 63 | 65 | 66 | 67 => ("rain", "Rain"),
        71 | 73 | 75 | 77 => ("snow", "Snow"),
        80..=82 => ("showers", "Showers"),
        85 | 86 => ("snow", "Snow Showers"),
        95 | 96 | 99 => ("thunder", "Thunderstorm"),
        _ => ("cloudy", "Cloudy"),
    }
}
