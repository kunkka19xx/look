//! Current conditions via Open-Meteo: geocode the place name, then fetch the
//! current forecast. Two sequential calls, both keyless. The geocoder can have a
//! multi-second cold start, hence the longer timeout.

use crate::{http, json::ValueExt, types::Answer};

const TIMEOUT_SECS: u32 = 6;

pub fn answer(place: &str) -> Option<Answer> {
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

struct Geo {
    lat: f64,
    lon: f64,
    name: String,
    country: String,
}

/// Resolves a place name to coordinates, tolerating trailing qualifier words the
/// geocoder rejects. Open-Meteo's geocoder matches a single place name, so
/// "haiphong vietnam" returns nothing while "haiphong" resolves - try the full
/// string first (so multi-word cities like "san francisco" still match), then
/// drop trailing words until something hits. Commas are treated as spaces.
fn geocode(place: &str) -> Option<Geo> {
    let normalized = place.replace(',', " ");
    let words: Vec<&str> = normalized.split_whitespace().collect();
    for end in (1..=words.len()).rev() {
        let candidate = words[..end].join(" ");
        if let Some(geo) = geocode_one(&candidate) {
            return Some(geo);
        }
    }
    None
}

fn geocode_one(name_query: &str) -> Option<Geo> {
    let geo = http::get_json(
        &format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1",
            http::encode(name_query),
        ),
        TIMEOUT_SECS,
    )?;
    let first = geo.get_arr("results")?.first()?;
    Some(Geo {
        lat: first.get_f64("latitude")?,
        lon: first.get_f64("longitude")?,
        name: first.get_str("name").unwrap_or(name_query).to_string(),
        country: first.get_str("country").unwrap_or("").to_string(),
    })
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
