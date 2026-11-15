//! Solar events, computed with the NOAA solar position algorithm.
//!
//! Standard arithmetic only: no external data, no network. The same numbers
//! Omarchy's own "elsewhen" panel draws are produced by its `Sun.js`; this
//! module exists so the tool works on any machine, with or without that
//! plugin.

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveTime, Offset, TimeZone};
use chrono_tz::Tz;

/// Official sunrise: the geometric zenith of the sun (90 degrees) plus
/// atmospheric refraction near the horizon and the sun's own radius.
const SUNRISE_ZENITH_DEG: f64 = 90.833;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Normal,
    /// The sun never sets.
    MidnightSun,
    /// The sun never rises.
    PolarNight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Between sunrise and sunset.
    Morning,
    /// Between sunset and the next sunrise.
    Evening,
}

#[derive(Debug, Clone)]
pub struct SolarEvents {
    pub kind: EventKind,
    pub sunrise: Option<DateTime<Tz>>,
    pub sunset: Option<DateTime<Tz>>,
    pub noon: DateTime<Tz>,
    pub day_length_minutes: Option<i64>,
}

/// Julian day at 00:00 UT of the given calendar date.
fn julian_day(day: NaiveDate) -> f64 {
    let (mut year, mut month) = (day.year(), day.month());
    if month <= 2 {
        year -= 1;
        month += 12;
    }
    let a = year.div_euclid(100);
    let b = 2 - a + a.div_euclid(4);
    let y = year as f64;
    let m = month as f64;
    (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + day.day() as f64 + b as f64
        - 1524.5
}

/// The sun's declination in degrees and the equation of time in minutes.
fn solar_parameters(jd: f64) -> (f64, f64) {
    let t = (jd - 2451545.0) / 36525.0;

    let mean_long = (280.46646 + t * (36000.76983 + t * 0.0003032)).rem_euclid(360.0);
    let mean_anom = 357.52911 + t * (35999.05029 - 0.0001537 * t);
    let ecc = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);

    let m = mean_anom.to_radians();
    let centre = m.sin() * (1.914602 - t * (0.004817 + 0.000014 * t))
        + (2.0 * m).sin() * (0.019993 - 0.000101 * t)
        + (3.0 * m).sin() * 0.000289;

    let true_long = mean_long + centre;
    let omega = 125.04 - 1934.136 * t;
    let app_long = true_long - 0.00569 - 0.00478 * omega.to_radians().sin();

    let mean_obliq =
        23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.0) / 60.0;
    let obliq = mean_obliq + 0.00256 * omega.to_radians().cos();

    let declination = (obliq.to_radians().sin() * app_long.to_radians().sin())
        .asin()
        .to_degrees();

    let y = (obliq / 2.0).to_radians().tan().powi(2);
    let l0 = mean_long.to_radians();
    let eq_time = 4.0
        * (y * (2.0 * l0).sin() - 2.0 * ecc * m.sin() + 4.0 * ecc * y * m.sin() * (2.0 * l0).cos()
            - 0.5 * y * y * (4.0 * l0).sin()
            - 1.25 * ecc * ecc * (2.0 * m).sin())
        .to_degrees();

    (declination, eq_time)
}

/// A wall clock time on the given day, in minutes after local midnight.
fn local_instant(tz: &Tz, day: NaiveDate, minutes: f64) -> DateTime<Tz> {
    let naive =
        day.and_time(NaiveTime::MIN) + Duration::milliseconds((minutes * 60_000.0).round() as i64);

    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        // A repeated wall clock time: the first one is the earlier instant.
        LocalResult::Ambiguous(first, _) => first,
        // A wall clock time inside a spring forward gap does not exist. Keep
        // the offset that was in effect before the transition, which is what
        // adding a duration to an aware timestamp does elsewhere.
        LocalResult::None => {
            let probe = tz
                .from_local_datetime(&(naive - Duration::hours(2)))
                .earliest()
                .or_else(|| tz.from_local_datetime(&naive).earliest());
            let offset = probe
                .map(|dt| dt.offset().fix().local_minus_utc())
                .unwrap_or(0);
            let utc = naive - Duration::seconds(offset as i64);
            tz.from_utc_datetime(&utc)
        }
    }
}

/// Sunrise, sunset and solar noon for a calendar day at a place.
///
/// `latitude` is north positive, `longitude` east positive.
pub fn solar_events(tz: &Tz, day: NaiveDate, latitude: f64, longitude: f64) -> SolarEvents {
    let jd_midnight = julian_day(day);

    // The offset of the zone from UTC, in minutes, at local noon of that day.
    let offset_minutes =
        match tz.from_local_datetime(&day.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap())) {
            // local_minus_utc is in seconds.
            LocalResult::Single(dt) => dt.offset().fix().local_minus_utc() / 60,
            LocalResult::Ambiguous(first, _) => first.offset().fix().local_minus_utc() / 60,
            LocalResult::None => 0,
        } as f64;

    // The sun's declination depends on the instant it is evaluated at, and the
    // instant that matters is the transit, which itself depends on the
    // declination. Solving it once at local midnight is good to about two
    // minutes of day length at high latitude, because the declination moves
    // most quickly near the equinoxes. Two rounds around the loop settle it.
    let (mut declination, mut eq_time) = solar_parameters(jd_midnight);
    let mut noon_minutes = 720.0 - 4.0 * longitude - eq_time + offset_minutes;
    for _ in 0..2 {
        let jd_noon = jd_midnight + (noon_minutes - offset_minutes) / 1440.0;
        let (d, e) = solar_parameters(jd_noon);
        declination = d;
        eq_time = e;
        noon_minutes = 720.0 - 4.0 * longitude - eq_time + offset_minutes;
    }

    let (lat, dec) = (latitude.to_radians(), declination.to_radians());
    let cos_ha =
        (SUNRISE_ZENITH_DEG.to_radians().cos() / (lat.cos() * dec.cos())) - lat.tan() * dec.tan();

    let noon = local_instant(tz, day, noon_minutes);

    if cos_ha < -1.0 {
        return SolarEvents {
            kind: EventKind::MidnightSun,
            sunrise: None,
            sunset: None,
            noon,
            day_length_minutes: Some(1440),
        };
    }
    if cos_ha > 1.0 {
        return SolarEvents {
            kind: EventKind::PolarNight,
            sunrise: None,
            sunset: None,
            noon,
            day_length_minutes: Some(0),
        };
    }

    let ha_degrees = cos_ha.acos().to_degrees();
    let sunrise_minutes = noon_minutes - ha_degrees * 4.0;
    let sunset_minutes = noon_minutes + ha_degrees * 4.0;

    SolarEvents {
        kind: EventKind::Normal,
        sunrise: Some(local_instant(tz, day, sunrise_minutes)),
        sunset: Some(local_instant(tz, day, sunset_minutes)),
        noon,
        day_length_minutes: Some(((sunset_minutes - sunrise_minutes).round()) as i64),
    }
}

/// Which half of the day `now` falls in.
///
/// The offsets shift sunrise and sunset by minutes, so a theme can change a
/// little before the sun actually appears.
pub fn phase_at(
    now: DateTime<Tz>,
    events: &SolarEvents,
    dawn_offset_minutes: i32,
    dusk_offset_minutes: i32,
) -> Option<Phase> {
    let sunrise = events.sunrise? + Duration::minutes(dawn_offset_minutes as i64);
    let sunset = events.sunset? + Duration::minutes(dusk_offset_minutes as i64);

    Some(if now >= sunrise && now < sunset {
        Phase::Morning
    } else {
        Phase::Evening
    })
}

/// The timezone the system is set to, read from the `/etc/localtime` symlink.
///
/// Falls back to the current offset when the zone cannot be named, which is
/// correct everywhere without daylight saving time.
pub fn system_timezone() -> Option<Tz> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let path = target.to_string_lossy().to_string();
    let name = path.split("zoneinfo/").nth(1)?;
    name.parse::<Tz>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{DAY_LENGTHS, REFERENCE};

    fn tz(name: &str) -> Tz {
        name.parse().unwrap()
    }

    fn minutes_of_day(dt: DateTime<Tz>) -> f64 {
        use chrono::Timelike;
        dt.hour() as f64 * 60.0 + dt.minute() as f64 + dt.second() as f64 / 60.0
    }

    fn parse_hhmm(text: &str) -> f64 {
        let (h, m) = text.split_once(':').unwrap();
        h.parse::<f64>().unwrap() * 60.0 + m.parse::<f64>().unwrap()
    }

    #[test]
    fn reference_values() {
        for (place, day, rise, set, lat, lon, zone, tolerance) in REFERENCE {
            let day: NaiveDate = day.parse().expect("date");
            let events = solar_events(&tz(zone), day, *lat, *lon);
            let sunrise = events.sunrise.expect("sunrise");
            let sunset = events.sunset.expect("sunset");
            let drift = (minutes_of_day(sunrise) - parse_hhmm(rise)).abs();
            assert!(
                drift <= *tolerance as f64,
                "{place} {day} sunrise: expected {rise}, got {}",
                sunrise.format("%H:%M")
            );
            let drift = (minutes_of_day(sunset) - parse_hhmm(set)).abs();
            assert!(
                drift <= *tolerance as f64,
                "{place} {day} sunset: expected {set}, got {}",
                sunset.format("%H:%M")
            );
        }
    }

    #[test]
    fn day_lengths() {
        for (day, expected, lat, lon, zone) in DAY_LENGTHS {
            let day: NaiveDate = day.parse().expect("date");
            let events = solar_events(&tz(zone), day, *lat, *lon);
            let length = events.day_length_minutes.expect("day length");
            assert!(
                (length - expected).abs() <= 1,
                "{zone} {day} day length: expected {expected}, got {length}"
            );
        }
    }

    #[test]
    fn polar_day() {
        let events = solar_events(
            &tz("Europe/Oslo"),
            NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
            69.65,
            18.96,
        );
        assert_eq!(events.kind, EventKind::MidnightSun);
        assert!(events.sunrise.is_none());
        assert_eq!(events.day_length_minutes, Some(1440));
    }

    #[test]
    fn polar_night() {
        let events = solar_events(
            &tz("UTC"),
            NaiveDate::from_ymd_opt(2026, 12, 15).unwrap(),
            89.0,
            0.0,
        );
        assert_eq!(events.kind, EventKind::PolarNight);
        assert_eq!(events.day_length_minutes, Some(0));
    }

    #[test]
    fn phase_switches_around_the_sun() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let events = solar_events(&tz("Asia/Shanghai"), day, 22.97, 113.74);
        let sunrise = events.sunrise.unwrap();
        let sunset = events.sunset.unwrap();
        let noon = events.noon;

        assert_eq!(
            phase_at(sunrise - Duration::hours(3), &events, 0, 0),
            Some(Phase::Evening)
        );
        assert_eq!(phase_at(noon, &events, 0, 0), Some(Phase::Morning));
        assert_eq!(
            phase_at(sunset + Duration::hours(3), &events, 0, 0),
            Some(Phase::Evening)
        );
    }

    #[test]
    fn dawn_offset_moves_the_switch() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let events = solar_events(&tz("Asia/Shanghai"), day, 22.97, 113.74);
        let sunrise = events.sunrise.unwrap();

        let half_past = sunrise + Duration::minutes(30);
        let late = sunrise + Duration::minutes(90);

        assert_eq!(phase_at(half_past, &events, 0, 0), Some(Phase::Morning));
        assert_eq!(
            phase_at(half_past, &events, 60, 0),
            Some(Phase::Evening),
            "a +60 minute dawn offset should not have switched yet"
        );
        assert_eq!(
            phase_at(late, &events, 60, 0),
            Some(Phase::Morning),
            "a +60 minute dawn offset should have switched by now"
        );
    }
}
