//! Reference values for the solar calculation.
//!
//! The expected times come from `astral`, an independent implementation of the
//! same NOAA algorithm, and the day lengths come from the sunrise-sunset.org
//! API, which is independent again. The table lives outside the test module so
//! that `omarchy-daynight selftest` can check the shipped binary too.

use chrono::NaiveDate;
use chrono_tz::Tz;

use crate::solar::{EventKind, Phase, phase_at, solar_events};
use std::time::Duration;

/// (place, day, sunrise, sunset, latitude, longitude, timezone, tolerance in minutes)
pub type ReferenceEntry = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    f64,
    f64,
    &'static str,
    u32,
);

/// (day, expected day length in minutes, latitude, longitude, timezone)
pub type DayLengthEntry = (&'static str, i64, f64, f64, &'static str);

pub const REFERENCE: &[ReferenceEntry] = &[
    (
        "Dongguan",
        "2026-01-15",
        "07:08",
        "18:00",
        22.97,
        113.74,
        "Asia/Shanghai",
        2,
    ),
    (
        "Dongguan",
        "2026-04-15",
        "06:05",
        "18:45",
        22.97,
        113.74,
        "Asia/Shanghai",
        2,
    ),
    (
        "Dongguan",
        "2026-07-15",
        "05:48",
        "19:13",
        22.97,
        113.74,
        "Asia/Shanghai",
        2,
    ),
    (
        "Dongguan",
        "2026-10-15",
        "06:21",
        "17:59",
        22.97,
        113.74,
        "Asia/Shanghai",
        2,
    ),
    (
        "Beijing",
        "2026-01-15",
        "07:34",
        "17:13",
        39.90,
        116.40,
        "Asia/Shanghai",
        2,
    ),
    (
        "Beijing",
        "2026-04-15",
        "05:37",
        "18:52",
        39.90,
        116.40,
        "Asia/Shanghai",
        2,
    ),
    (
        "Beijing",
        "2026-07-15",
        "04:58",
        "19:41",
        39.90,
        116.40,
        "Asia/Shanghai",
        2,
    ),
    (
        "Beijing",
        "2026-10-15",
        "06:24",
        "17:35",
        39.90,
        116.40,
        "Asia/Shanghai",
        2,
    ),
    (
        "Urumqi",
        "2026-01-15",
        "09:40",
        "18:57",
        43.83,
        87.62,
        "Asia/Shanghai",
        2,
    ),
    (
        "Urumqi",
        "2026-04-15",
        "07:27",
        "20:52",
        43.83,
        87.62,
        "Asia/Shanghai",
        2,
    ),
    (
        "Urumqi",
        "2026-07-15",
        "06:41",
        "21:49",
        43.83,
        87.62,
        "Asia/Shanghai",
        2,
    ),
    (
        "Urumqi",
        "2026-10-15",
        "08:23",
        "19:26",
        43.83,
        87.62,
        "Asia/Shanghai",
        2,
    ),
    (
        "Sydney",
        "2026-01-15",
        "05:59",
        "20:08",
        -33.87,
        151.21,
        "Australia/Sydney",
        2,
    ),
    (
        "Sydney",
        "2026-04-15",
        "06:17",
        "17:32",
        -33.87,
        151.21,
        "Australia/Sydney",
        2,
    ),
    (
        "Sydney",
        "2026-07-15",
        "06:58",
        "17:04",
        -33.87,
        151.21,
        "Australia/Sydney",
        2,
    ),
    (
        "Sydney",
        "2026-10-15",
        "06:14",
        "19:08",
        -33.87,
        151.21,
        "Australia/Sydney",
        2,
    ),
    (
        "NewYork",
        "2026-01-15",
        "07:18",
        "16:53",
        40.71,
        -74.01,
        "America/New_York",
        2,
    ),
    (
        "NewYork",
        "2026-04-15",
        "06:17",
        "19:35",
        40.71,
        -74.01,
        "America/New_York",
        2,
    ),
    (
        "NewYork",
        "2026-07-15",
        "05:38",
        "20:25",
        40.71,
        -74.01,
        "America/New_York",
        2,
    ),
    (
        "NewYork",
        "2026-10-15",
        "07:07",
        "18:15",
        40.71,
        -74.01,
        "America/New_York",
        2,
    ),
    // High latitude: the sun's path is shallow, so the same algorithm drifts a
    // little more here.
    (
        "Reykjavik",
        "2026-01-15",
        "10:55",
        "16:19",
        64.15,
        -21.94,
        "Atlantic/Reykjavik",
        3,
    ),
    (
        "Reykjavik",
        "2026-04-15",
        "05:56",
        "21:01",
        64.15,
        -21.94,
        "Atlantic/Reykjavik",
        3,
    ),
    (
        "Reykjavik",
        "2026-10-15",
        "08:18",
        "18:07",
        64.15,
        -21.94,
        "Atlantic/Reykjavik",
        3,
    ),
];

/// (day, expected day length in minutes, latitude, longitude, timezone)
pub const DAY_LENGTHS: &[DayLengthEntry] = &[
    ("2026-10-15", 590, 64.15, -21.94, "Atlantic/Reykjavik"),
    ("2026-01-15", 326, 64.15, -21.94, "Atlantic/Reykjavik"),
    ("2026-07-15", 1184, 64.15, -21.94, "Atlantic/Reykjavik"),
    ("2026-10-15", 669, 40.71, -74.01, "America/New_York"),
    ("2026-01-15", 576, 40.71, -74.01, "America/New_York"),
    ("2026-10-15", 698, 22.97, 113.74, "Asia/Shanghai"),
    ("2026-01-15", 653, 22.97, 113.74, "Asia/Shanghai"),
    ("2026-10-15", 671, 39.90, 116.40, "Asia/Shanghai"),
];

fn parse_hhmm(text: &str) -> f64 {
    let (h, m) = text.split_once(':').expect("hh:mm");
    h.parse::<f64>().unwrap() * 60.0 + m.parse::<f64>().unwrap()
}

fn minutes_of_day(dt: chrono::DateTime<Tz>) -> f64 {
    use chrono::Timelike;
    dt.hour() as f64 * 60.0 + dt.minute() as f64 + dt.second() as f64 / 60.0
}

/// Check the calculation against the reference tables.
///
/// Returns the number of checks that failed.
pub fn run_selftest() -> (usize, Vec<String>) {
    let mut checks = 0usize;
    let mut failures = Vec::new();
    let mut worst_drift = 0.0f64;

    for (place, day, rise, set, lat, lon, zone, tolerance) in REFERENCE {
        let zone: Tz = zone.parse().expect("timezone");
        let day: NaiveDate = day.parse().expect("date");
        let events = solar_events(&zone, day, *lat, *lon);

        for (label, expected_text, actual) in [
            ("sunrise", rise, events.sunrise),
            ("sunset", set, events.sunset),
        ] {
            checks += 1;
            let actual = match actual {
                Some(dt) => dt,
                None => {
                    failures.push(format!("{place} {day} {label}: no event"));
                    continue;
                }
            };
            let drift = (minutes_of_day(actual) - parse_hhmm(expected_text)).abs();
            worst_drift = worst_drift.max(drift);
            if drift > *tolerance as f64 {
                failures.push(format!(
                    "{place} {day} {label}: expected {expected_text}, got {} ({drift:.1} min off)",
                    actual.format("%H:%M")
                ));
            }
        }
    }

    for (day, expected, lat, lon, zone) in DAY_LENGTHS {
        checks += 1;
        let zone: Tz = zone.parse().expect("timezone");
        let day: NaiveDate = day.parse().expect("date");
        let events = solar_events(&zone, day, *lat, *lon);
        match events.day_length_minutes {
            Some(length) if (length - expected).abs() <= 1 => {}
            Some(length) => failures.push(format!(
                "{zone} {day} day length: expected {expected} min, got {length} min"
            )),
            None => failures.push(format!("{zone} {day} day length: missing")),
        }
    }

    // Polar day and polar night.
    checks += 2;
    let tromso = solar_events(
        &"Europe/Oslo".parse().unwrap(),
        NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
        69.65,
        18.96,
    );
    if tromso.kind != EventKind::MidnightSun {
        failures.push("Tromso in June should report midnight_sun".into());
    }
    let pole = solar_events(
        &"UTC".parse().unwrap(),
        NaiveDate::from_ymd_opt(2026, 12, 15).unwrap(),
        89.0,
        0.0,
    );
    if pole.kind != EventKind::PolarNight {
        failures.push("the north pole in December should report polar_night".into());
    }

    // The phase flips at sunrise and at sunset, and the dawn offset moves it.
    checks += 5;
    let events = solar_events(
        &"Asia/Shanghai".parse().unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 15).unwrap(),
        22.97,
        113.74,
    );
    let sunrise = events.sunrise.expect("sunrise");
    let sunset = events.sunset.expect("sunset");
    if phase_at(sunrise - Duration::from_secs(3600 * 3), &events, 0, 0) != Some(Phase::Evening) {
        failures.push("before sunrise should be evening".into());
    }
    if phase_at(events.noon, &events, 0, 0) != Some(Phase::Morning) {
        failures.push("midday should be morning".into());
    }
    if phase_at(sunset + Duration::from_secs(3600 * 3), &events, 0, 0) != Some(Phase::Evening) {
        failures.push("after sunset should be evening".into());
    }
    if phase_at(sunrise + Duration::from_secs(1800), &events, 60, 0) != Some(Phase::Evening) {
        failures.push("a +60 minute dawn offset should not have switched yet".into());
    }
    if phase_at(sunrise + Duration::from_secs(5400), &events, 60, 0) != Some(Phase::Morning) {
        failures.push("a +60 minute dawn offset should have switched by now".into());
    }

    println!(
        "  reference values: {} days checked, worst drift {worst_drift:.2} min",
        REFERENCE.len()
    );
    println!(
        "  day lengths: {} cases checked against the API",
        DAY_LENGTHS.len()
    );
    println!("  polar day and polar night: checked");
    println!("  phase: before, during, after and dawn offsets checked");

    (checks, failures)
}
