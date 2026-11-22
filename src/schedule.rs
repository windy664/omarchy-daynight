//! Arming one-shot timers that fire exactly at sunrise and sunset.
//!
//! The poll timer cannot know when the sun will rise tomorrow, so it can
//! only check every `poll_interval` and switch up to that much late. This
//! module asks systemd for the same thing the poll would find out, but
//! schedules the switch at the exact moment instead of finding it later.
//!
//! Sunrise and sunset drift by a few minutes every day, so the timers are
//! re-armed every morning rather than being pinned to a fixed hour.

use std::process::{Command, Stdio};

use chrono::{DateTime, Duration};
use chrono_tz::Tz;

use crate::config::{Config, PROGRAM_NAME};
use crate::install::binary_path;
use crate::solar::{SolarEvents, system_timezone};

/// A one-shot timer this module asked systemd for.
pub struct Armed {
    pub label: &'static str,
    pub when: DateTime<Tz>,
}

/// The two moments the theme changes, shifted by the configured offsets.
fn switch_times(config: &Config, events: &SolarEvents) -> Vec<(&'static str, DateTime<Tz>)> {
    let mut times = Vec::new();

    if let Some(sunrise) = events.sunrise {
        times.push((
            "dawn",
            sunrise + Duration::minutes(config.dawn_offset as i64),
        ));
    }
    if let Some(sunset) = events.sunset {
        times.push((
            "dusk",
            sunset + Duration::minutes(config.dusk_offset as i64),
        ));
    }

    times
}

/// The name of the one-shot unit for a switch.
fn unit_name(label: &str, when: &DateTime<Tz>) -> String {
    format!("{PROGRAM_NAME}-{label}-{}", when.format("%Y%m%d"))
}

/// systemd reads `OnCalendar` in the system timezone, so the instant has to
/// be spelled out in that timezone even when the solar calculation ran
/// somewhere else.
fn on_calendar(when: &DateTime<Tz>) -> String {
    let system = system_timezone().unwrap_or_else(|| when.timezone().clone());
    when.with_timezone(&system)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Drop the one-shot timers left over from an earlier run.
///
/// They are transient units, so they are gone once they have fired; this
/// only matters when the scheduler runs twice before the switch happens.
fn clear_previous(label: &str, when: &DateTime<Tz>) {
    let unit = unit_name(label, when);
    systemctl(&["stop", &format!("{unit}.timer"), &format!("{unit}.service")]);
    systemctl(&[
        "reset-failed",
        &format!("{unit}.timer"),
        &format!("{unit}.service"),
    ]);
}

/// Ask systemd for a one-shot timer that runs `omarchy-daynight run`.
fn arm_one(label: &str, when: &DateTime<Tz>) -> Result<(), String> {
    let unit = unit_name(label, when);
    let when_text = on_calendar(when);

    clear_previous(label, when);

    let output = Command::new("systemd-run")
        .arg("--user")
        .arg("--collect")
        .arg(format!("--unit={unit}"))
        .arg(format!("--on-calendar={when_text}"))
        .arg("--timer-property=Persistent=true")
        .arg(format!(
            "--description={PROGRAM_NAME} {label} at {when_text}"
        ))
        .arg(binary_path())
        .arg("run")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not run systemd-run: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "could not arm the {label} timer: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

/// Arm the timers for the switches that are still ahead of us.
///
/// A switch that has already passed is left to the poll timer, which is
/// what catches up on a sunrise the machine was not awake for.
pub fn arm(config: &Config, events: &SolarEvents, now: DateTime<Tz>) -> Result<Vec<Armed>, String> {
    let mut armed = Vec::new();

    for (label, when) in switch_times(config, events) {
        if when <= now {
            continue;
        }

        arm_one(label, &when)?;
        armed.push(Armed { label, when });
    }

    Ok(armed)
}
