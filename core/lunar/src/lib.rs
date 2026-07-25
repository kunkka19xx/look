//! Solar-to-lunar date conversion for the East Asian lunisolar calendar
//! (Vietnamese am lich, Chinese nong li, ...). Pure arithmetic, no dependency
//! beyond serde for the result type - the shared source of truth every shell
//! reads (linows over IPC, macOS over the FFI bridge).
//!
//! Ported from Ho Ngoc Duc's public-domain algorithm (astronomical new-moon and
//! solar-longitude approximations, https://www.informatik.uni-leipzig.de/~duc/amlich/).
//! The conversion is timezone-sensitive - a new moon falls on a different civil
//! day either side of a meridian - so the caller passes the viewer's UTC offset
//! in hours: 7 yields the Vietnamese calendar, 8 the Chinese one.

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// A resolved lunar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LunarDate {
    pub day: i64,
    pub month: i64,
    pub year: i64,
    /// True when this is the intercalary (repeated) month of a 13-month year.
    pub leap: bool,
}

/// Julian day number for a Gregorian date, switching to the Julian calendar
/// before the 1582 reform (jd < 2299161).
fn jd_from_date(dd: i64, mm: i64, yy: i64) -> i64 {
    let a = (14 - mm) / 12;
    let y = yy + 4800 - a;
    let m = mm + 12 * a - 3;
    let jd = dd + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    if jd < 2299161 {
        dd + (153 * m + 2) / 5 + 365 * y + y / 4 - 32083
    } else {
        jd
    }
}

/// Civil day (integer JD at the given timezone) of the k-th new moon since the
/// 1900 epoch.
fn new_moon_day(k: i64, time_zone: f64) -> i64 {
    let k = k as f64;
    let t = k / 1236.85;
    let t2 = t * t;
    let t3 = t2 * t;
    let dr = PI / 180.0;
    let mut jd1 = 2415020.75933 + 29.53058868 * k + 0.0001178 * t2 - 0.000000155 * t3;
    jd1 += 0.00033 * ((166.56 + 132.87 * t - 0.009173 * t2) * dr).sin();
    let m = 359.2242 + 29.10535608 * k - 0.0000333 * t2 - 0.00000347 * t3;
    let mpr = 306.0253 + 385.81691806 * k + 0.0107306 * t2 + 0.00001236 * t3;
    let f = 21.2964 + 390.67050646 * k - 0.0016528 * t2 - 0.00000239 * t3;
    let mut c1 = (0.1734 - 0.000393 * t) * (m * dr).sin() + 0.0021 * (2.0 * dr * m).sin();
    c1 = c1 - 0.4068 * (mpr * dr).sin() + 0.0161 * (dr * 2.0 * mpr).sin();
    c1 -= 0.0004 * (dr * 3.0 * mpr).sin();
    c1 = c1 + 0.0104 * (dr * 2.0 * f).sin() - 0.0051 * (dr * (m + mpr)).sin();
    c1 = c1 - 0.0074 * (dr * (m - mpr)).sin() + 0.0004 * (dr * (2.0 * f + m)).sin();
    c1 = c1 - 0.0004 * (dr * (2.0 * f - m)).sin() - 0.0006 * (dr * (2.0 * f + mpr)).sin();
    c1 = c1 + 0.001 * (dr * (2.0 * f - mpr)).sin() + 0.0005 * (dr * (2.0 * mpr + m)).sin();
    let deltat = if t < -11.0 {
        0.001 + 0.000839 * t + 0.0002261 * t2 - 0.00000845 * t3 - 0.000000081 * t * t3
    } else {
        -0.000278 + 0.000265 * t + 0.000262 * t2
    };
    let jd_new = jd1 + c1 - deltat;
    (jd_new + 0.5 + time_zone / 24.0).floor() as i64
}

/// Solar longitude sextant (0..11) at the start of the given civil day: which
/// 30-degree arc of the ecliptic the sun occupies, used to place month 11.
fn sun_longitude(jdn: i64, time_zone: f64) -> i64 {
    let t = (jdn as f64 - 2451545.5 - time_zone / 24.0) / 36525.0;
    let t2 = t * t;
    let dr = PI / 180.0;
    let m = 357.5291 + 35999.0503 * t - 0.0001559 * t2 - 0.00000048 * t * t2;
    let l0 = 280.46645 + 36000.76983 * t + 0.0003032 * t2;
    let mut dl = (1.9146 - 0.004817 * t - 0.000014 * t2) * (dr * m).sin();
    dl = dl + (0.019993 - 0.000101 * t) * (dr * 2.0 * m).sin() + 0.00029 * (dr * 3.0 * m).sin();
    let mut l = (l0 + dl) * dr;
    l -= PI * 2.0 * (l / (PI * 2.0)).floor();
    (l / PI * 6.0).floor() as i64
}

/// Civil day of the new moon that begins lunar month 11 (the month containing
/// the winter solstice) for the given solar year.
fn lunar_month_11(yy: i64, time_zone: f64) -> i64 {
    let off = jd_from_date(31, 12, yy) - 2415021;
    let k = (off as f64 / 29.530588853).floor() as i64;
    let nm = new_moon_day(k, time_zone);
    if sun_longitude(nm, time_zone) >= 9 {
        new_moon_day(k - 1, time_zone)
    } else {
        nm
    }
}

/// Offset (in months after month 11) of the leap month in a 13-month lunar year.
fn leap_month_offset(a11: i64, time_zone: f64) -> i64 {
    let k = ((a11 as f64 - 2415021.076998695) / 29.530588853 + 0.5).floor() as i64;
    let mut i = 1;
    let mut arc = sun_longitude(new_moon_day(k + i, time_zone), time_zone);
    loop {
        let last = arc;
        i += 1;
        arc = sun_longitude(new_moon_day(k + i, time_zone), time_zone);
        if arc == last || i >= 14 {
            break;
        }
    }
    i - 1
}

/// Convert a Gregorian date to its lunar date at the given UTC offset (hours).
pub fn solar_to_lunar(year: i64, month: i64, day: i64, time_zone: f64) -> LunarDate {
    let day_number = jd_from_date(day, month, year);
    let k = ((day_number as f64 - 2415021.076998695) / 29.530588853).floor() as i64;
    let mut month_start = new_moon_day(k + 1, time_zone);
    if month_start > day_number {
        month_start = new_moon_day(k, time_zone);
    }
    let mut a11 = lunar_month_11(year, time_zone);
    let mut b11 = a11;
    let mut lunar_year;
    if a11 >= month_start {
        lunar_year = year;
        a11 = lunar_month_11(year - 1, time_zone);
    } else {
        lunar_year = year + 1;
        b11 = lunar_month_11(year + 1, time_zone);
    }
    let lunar_day = day_number - month_start + 1;
    let diff = ((month_start - a11) as f64 / 29.0).floor() as i64;
    let mut lunar_leap = false;
    let mut lunar_month = diff + 11;
    if b11 - a11 > 365 {
        let leap_offset = leap_month_offset(a11, time_zone);
        if diff >= leap_offset {
            lunar_month = diff + 10;
            if diff == leap_offset {
                lunar_leap = true;
            }
        }
    }
    if lunar_month > 12 {
        lunar_month -= 12;
    }
    if lunar_month >= 11 && diff < 4 {
        lunar_year -= 1;
    }
    LunarDate {
        day: lunar_day,
        month: lunar_month,
        year: lunar_year,
        leap: lunar_leap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vietnamese calendar (UTC+7).
    const VN: f64 = 7.0;

    fn ymd(year: i64, month: i64, day: i64) -> (i64, i64) {
        let l = solar_to_lunar(year, month, day, VN);
        (l.day, l.month)
    }

    #[test]
    fn lunar_new_year_is_first_of_first_month() {
        // Tet (lunar new year) for three consecutive years.
        assert_eq!(ymd(2024, 2, 10), (1, 1));
        assert_eq!(ymd(2025, 1, 29), (1, 1));
        assert_eq!(ymd(2026, 2, 17), (1, 1));
    }

    #[test]
    fn full_moon_is_fifteenth() {
        // Ram thang 3, 2025.
        assert_eq!(ymd(2025, 4, 12), (15, 3));
    }

    #[test]
    fn known_ordinary_day() {
        assert_eq!(ymd(2026, 7, 25), (12, 6));
    }

    #[test]
    fn carries_lunar_year() {
        let l = solar_to_lunar(2026, 2, 17, VN);
        assert_eq!(l.year, 2026);
    }
}
