//! omarchy-daynight: switch the Omarchy theme at real sunrise and sunset.

mod config;
mod install;
mod locate;
mod reference;
mod schedule;
mod solar;
mod theme;

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use std::process::{Command, Stdio};

use config::{Config, PROGRAM_NAME};
use solar::{EventKind, Phase, phase_at, solar_events};

fn usage() {
    println!(
        r#"{PROGRAM_NAME} -- Omarchy themes that follow the sun

Usage: {PROGRAM_NAME} <command>

Commands:
  run         Switch the theme if the phase changed since the last run
  status      Show today's sunrise, sunset, current theme and next switch
  schedule    Arm one-shot timers that fire exactly at sunrise and sunset
  install     Install the config, the systemd user units and enable the timer
  uninstall   Disable the timer and remove the systemd user units
  locate      Guess your coordinates from your public IP address
  selftest    Check the solar calculation against reference values

Config: {}"#,
        config::config_file().display()
    );
}

/// The timezone, the coordinates and today's sun events.
struct Day {
    timezone: Tz,
    latitude: f64,
    longitude: f64,
    events: solar::SolarEvents,
}

fn today(tz: &Tz) -> chrono::NaiveDate {
    Utc::now().with_timezone(tz).date_naive()
}

fn day_for(config: &Config) -> Result<Day, String> {
    let (Some(latitude), Some(longitude)) = (config.latitude, config.longitude) else {
        return Err(format!(
            "no coordinates configured; run '{PROGRAM_NAME} locate' or edit {}",
            config::config_file().display()
        ));
    };

    let timezone = config.resolve_timezone();
    let events = solar_events(&timezone, today(&timezone), latitude, longitude);

    Ok(Day {
        timezone,
        latitude,
        longitude,
        events,
    })
}

/// Which half of the day it is, falling back to fixed hours inside the polar
/// circles where the sun may never rise or set.
fn current_phase(config: &Config, day: &Day) -> Phase {
    if day.events.kind == EventKind::Normal {
        let now: DateTime<Tz> = Utc::now().with_timezone(&day.timezone);
        if let Some(phase) = phase_at(now, &day.events, config.dawn_offset, config.dusk_offset) {
            return phase;
        }
    }

    let now = Utc::now().with_timezone(&day.timezone);
    let minutes = now.hour() as i32 * 60 + now.minute() as i32;
    let day_start = parse_hhmm(&config.fallback_day_start);
    let night_start = parse_hhmm(&config.fallback_night_start);

    if minutes >= day_start && minutes < night_start {
        Phase::Morning
    } else {
        Phase::Evening
    }
}

fn parse_hhmm(text: &str) -> i32 {
    let mut parts = text.split(':');
    let hours = parts
        .next()
        .and_then(|h| h.trim().parse().ok())
        .unwrap_or(0);
    let minutes = parts
        .next()
        .and_then(|m| m.trim().parse().ok())
        .unwrap_or(0);
    hours * 60 + minutes
}

fn wanted_theme(config: &Config, phase: Phase) -> String {
    match phase {
        Phase::Morning => config.day_theme.clone(),
        Phase::Evening => config.night_theme.clone(),
    }
}

fn run_command() -> Result<(), String> {
    let mut config = Config::load();

    if config.auto_locate
        && let Ok(location) = locate::locate()
    {
        config.latitude = Some(location.latitude);
        config.longitude = Some(location.longitude);
    }

    let day = day_for(&config)?;
    let phase = current_phase(&config, &day);
    let wanted = wanted_theme(&config, phase);
    let current = theme::current_theme().unwrap_or_else(|| "unknown".into());

    if config::pause_file().exists() {
        println!(
            "{PROGRAM_NAME}: paused ({} exists), wanted {wanted}, current {current}",
            config::pause_file().display()
        );
        return Ok(());
    }

    if current == wanted {
        println!("{PROGRAM_NAME}: already on {wanted}");
        return Ok(());
    }

    println!("{PROGRAM_NAME}: switching {current} -> {wanted}");
    theme::switch_theme(&wanted)?;

    if config.nightlight == "auto" {
        theme::set_nightlight(phase == Phase::Evening);
    }

    if !config.on_switch.is_empty() {
        let _ = Command::new("sh")
            .arg("-c")
            .arg(&config.on_switch)
            .stdin(Stdio::null())
            .status();
    }

    theme::notify("Day/night theme", &format!("Switched to {wanted}"));
    Ok(())
}

fn status_command() -> Result<(), String> {
    let config = Config::load();
    let day = day_for(&config)?;
    let phase = current_phase(&config, &day);
    let wanted = wanted_theme(&config, phase);
    let current = theme::current_theme().unwrap_or_else(|| "unknown".into());

    println!(
        "location   {}, {} ({})",
        day.latitude,
        day.longitude,
        config.timezone_label()
    );
    println!(
        "sunrise    {}",
        day.events
            .sunrise
            .map(|t| t.format("%Y-%m-%dT%H:%M:%S%z").to_string())
            .unwrap_or_else(|| "-".into())
    );
    println!(
        "sunset     {}",
        day.events
            .sunset
            .map(|t| t.format("%Y-%m-%dT%H:%M:%S%z").to_string())
            .unwrap_or_else(|| "-".into())
    );
    println!(
        "phase      {}",
        match phase {
            Phase::Morning => "morning",
            Phase::Evening => "evening",
        }
    );
    println!("theme now  {current}");
    println!("theme want {wanted}");
    if config::pause_file().exists() {
        println!("state      paused");
    }
    println!(
        "timer      {}",
        if install::timer_active() {
            "active"
        } else {
            "inactive"
        }
    );

    Ok(())
}

/// Arm the exact sunrise and sunset timers.
///
/// This is what the schedule timer runs every morning. It is also useful on
/// its own, after the coordinates or the offsets in the config have changed.
fn schedule_command() -> Result<(), String> {
    let mut config = Config::load();

    if config.auto_locate
        && let Ok(location) = locate::locate()
    {
        config.latitude = Some(location.latitude);
        config.longitude = Some(location.longitude);
    }

    let day = day_for(&config)?;
    let now = Utc::now().with_timezone(&day.timezone);
    let armed = schedule::arm(&config, &day.events, now)?;

    if armed.is_empty() {
        println!("{PROGRAM_NAME}: no switch ahead of us to arm");
        return Ok(());
    }

    for timer in &armed {
        let label = timer.label;
        let when = timer.when.format("%Y-%m-%d %H:%M:%S %z").to_string();
        println!("{PROGRAM_NAME}: armed the {label} timer for {when}");
    }

    Ok(())
}

fn install_command(migrate_legacy: bool) -> Result<(), String> {
    let mut config = Config::load();

    if config.latitude.is_none() || config.longitude.is_none() {
        println!("{PROGRAM_NAME}: looking up your coordinates...");
        match locate::locate() {
            Ok(location) => {
                config.latitude = Some(location.latitude);
                config.longitude = Some(location.longitude);
                println!(
                    "{PROGRAM_NAME}: found {} ({}, {})",
                    location.city, location.latitude, location.longitude
                );
            }
            Err(error) => {
                eprintln!("{PROGRAM_NAME}: {error}");
                println!(
                    "{PROGRAM_NAME}: edit {} by hand",
                    config::config_file().display()
                );
            }
        }
    }

    config
        .save()
        .map_err(|error| format!("could not write the config: {error}"))?;
    install::install(&mut config, migrate_legacy)?;

    println!();
    println!("{PROGRAM_NAME} installed.");
    println!("  config  {}", config::config_file().display());
    if config.precise_schedule {
        println!(
            "  timer   {} (every {}, exact switches armed by {})",
            install::TIMER_NAME,
            config.fallback_interval,
            install::SCHEDULE_TIMER_NAME
        );
    } else {
        println!(
            "  timer   {} (every {})",
            install::TIMER_NAME,
            config.poll_interval
        );
    }
    println!();

    status_command().ok();
    Ok(())
}

fn locate_command() -> Result<(), String> {
    let location = locate::locate()?;
    println!("{}", location.latitude);
    println!("{}", location.longitude);
    println!("{}", location.city);
    Ok(())
}

fn selftest_command() -> Result<(), String> {
    println!("solar self test");
    let (checks, failures) = reference::run_selftest();
    if failures.is_empty() {
        println!("\nall {checks} checks passed");
        Ok(())
    } else {
        eprintln!("\n{} of {checks} checks failed:", failures.len());
        for failure in &failures {
            eprintln!("  - {failure}");
        }
        Err("the solar calculation does not match the reference values".into())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");

    let result = match command {
        "run" => run_command(),
        "status" => status_command(),
        "schedule" => schedule_command(),
        "install" => install_command(args.get(1).map(String::as_str) == Some("--migrate-legacy")),
        "uninstall" => install::uninstall(),
        "locate" => locate_command(),
        "selftest" => selftest_command(),
        "" | "-h" | "--help" | "help" => {
            usage();
            return;
        }
        other => {
            eprintln!("{PROGRAM_NAME}: unknown command '{other}'");
            usage();
            std::process::exit(1);
        }
    };

    if let Err(error) = result {
        eprintln!("{PROGRAM_NAME}: {error}");
        std::process::exit(1);
    }
}
