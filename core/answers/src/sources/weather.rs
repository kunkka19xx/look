//! Current conditions via Open-Meteo: geocode the place name, then fetch the
//! current forecast. Two sequential calls, both keyless. The geocoder can have a
//! multi-second cold start, hence the longer timeout.
//!
//! Two caches keep the common case off the wire entirely. Coordinates never
//! change, so a resolved place is remembered for the life of the process; the
//! forecast is held for a few minutes, which is far shorter than the weather
//! takes to move and long enough to cover typing the same query twice. Without
//! them every keystroke-completed query paid for two cold round trips, and a
//! single slow one turned into "Couldn't find an answer".

use crate::{http, json::ValueExt, types::Answer};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const TIMEOUT_SECS: u32 = 6;
/// How long a fetched forecast stays fresh.
const FORECAST_TTL: Duration = Duration::from_secs(300);

static GEO_CACHE: LazyLock<Mutex<HashMap<String, Geo>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static FORECAST_CACHE: LazyLock<Mutex<HashMap<String, (Instant, Answer)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// The last place that resolved, so a bare `weather` means "same place as
/// before" instead of nothing at all.
static LAST_PLACE: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

fn cache_key(place: &str) -> String {
    place.trim().to_lowercase()
}

/// Whether a bare `weather` has somewhere to report on yet. The gate consults
/// this so the word alone doesn't claim a query before it can answer one.
pub fn has_remembered_place() -> bool {
    LAST_PLACE
        .lock()
        .map(|last| last.is_some())
        .unwrap_or(false)
}

fn remembered_place() -> Option<String> {
    LAST_PLACE.lock().ok()?.clone()
}

pub fn answer(place: &str) -> Option<Answer> {
    let place = &if place.trim().is_empty() {
        remembered_place()?
    } else {
        place.to_string()
    };
    let key = cache_key(place);
    if let Ok(cache) = FORECAST_CACHE.lock()
        && let Some((at, answer)) = cache.get(&key)
        && at.elapsed() < FORECAST_TTL
    {
        return Some(answer.clone());
    }
    let answer = fetch(place)?;
    if let Ok(mut cache) = FORECAST_CACHE.lock() {
        cache.insert(key, (Instant::now(), answer.clone()));
    }
    Some(answer)
}

fn fetch(place: &str) -> Option<Answer> {
    let geo = geocode(place)?;
    let (lat, lon) = (geo.lat, geo.lon);
    let name = geo.name.as_str();
    let country = geo.country.as_str();

    let forecast = http::get_json(
        &format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m",
        ),
        TIMEOUT_SECS,
    )?;
    let current = forecast.get("current")?;
    let temp = current.get_f64("temperature_2m")?;
    let code = current.get_i64("weather_code").unwrap_or(-1);

    let place_label = if country.is_empty() {
        name.to_string()
    } else {
        format!("{name}, {country}")
    };
    let mut text = format!("{place_label}: {}°C", temp.round() as i64);
    if let Some(feels) = current.get_f64("apparent_temperature")
        && (feels - temp).abs() >= 1.0
    {
        text.push_str(&format!(" (feels {}°C)", feels.round() as i64));
    }
    text.push_str(&format!(", {}.", wmo_description(code)));
    if let Some(humidity) = current.get_i64("relative_humidity_2m") {
        text.push_str(&format!(" Humidity {humidity}%."));
    }
    if let Some(wind) = current.get_f64("wind_speed_10m") {
        text.push_str(&format!(" Wind {} km/h.", wind.round() as i64));
    }
    Some(Answer::text(text, "Weather"))
}

#[derive(Clone)]
struct Geo {
    lat: f64,
    lon: f64,
    name: String,
    country: String,
}

/// A geocoder lookup: the place resolved, the geocoder answered but knows no
/// such place, or the request never completed. The last two look identical to
/// the caller of an `Option` and must not: one is final, the other is worth
/// asking again.
enum Lookup {
    Found(Geo),
    NotFound,
    Failed,
}

/// Coordinates for a place name, cached for the process lifetime - a city's
/// latitude is not news.
fn geocode(place: &str) -> Option<Geo> {
    let key = cache_key(place);
    if let Ok(cache) = GEO_CACHE.lock()
        && let Some(hit) = cache.get(&key)
    {
        return Some(hit.clone());
    }
    let geo = resolve(place)?;
    if let Ok(mut cache) = GEO_CACHE.lock() {
        cache.insert(key, geo.clone());
    }
    if let Ok(mut last) = LAST_PLACE.lock() {
        *last = Some(place.trim().to_string());
    }
    Some(geo)
}

/// Resolves a place name to coordinates, tolerating trailing qualifier words the
/// geocoder rejects. Open-Meteo's geocoder matches a single place name, so
/// "haiphong vietnam" returns nothing while "haiphong" resolves - try the full
/// string first (so multi-word cities like "san francisco" still match), then
/// drop trailing words until something hits. Commas are treated as spaces.
fn resolve(place: &str) -> Option<Geo> {
    let normalized = place.replace(',', " ");
    let words: Vec<&str> = normalized.split_whitespace().collect();
    for end in (1..=words.len()).rev() {
        let candidate = words[..end].join(" ");
        match geocode_one(&candidate) {
            Lookup::Found(geo) => return Some(geo),
            // A request that didn't complete is the geocoder's cold start, not
            // a misspelled city. One retry costs a second; giving up costs the
            // user the feature for a place that exists.
            Lookup::Failed => {
                if let Lookup::Found(geo) = geocode_one(&candidate) {
                    return Some(geo);
                }
            }
            Lookup::NotFound => {}
        }
    }
    None
}

fn geocode_one(name_query: &str) -> Lookup {
    let Some(geo) = http::get_json(
        &format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1",
            http::encode(name_query),
        ),
        TIMEOUT_SECS,
    ) else {
        return Lookup::Failed;
    };
    let Some(first) = geo.get_arr("results").and_then(|list| list.first()) else {
        return Lookup::NotFound;
    };
    match (first.get_f64("latitude"), first.get_f64("longitude")) {
        (Some(lat), Some(lon)) => Lookup::Found(Geo {
            lat,
            lon,
            name: first.get_str("name").unwrap_or(name_query).to_string(),
            country: first.get_str("country").unwrap_or("").to_string(),
        }),
        _ => Lookup::NotFound,
    }
}

/// WMO weather-interpretation codes -> short description.
fn wmo_description(code: i64) -> &'static str {
    match code {
        0 => "clear sky",
        1 => "mainly clear",
        2 => "partly cloudy",
        3 => "overcast",
        45 | 48 => "fog",
        51 | 53 | 55 => "drizzle",
        56 | 57 => "freezing drizzle",
        61 | 63 | 65 => "rain",
        66 | 67 => "freezing rain",
        71 | 73 | 75 => "snow",
        77 => "snow grains",
        80..=82 => "rain showers",
        85 | 86 => "snow showers",
        95 => "thunderstorm",
        96 | 99 => "thunderstorm with hail",
        _ => "-",
    }
}
