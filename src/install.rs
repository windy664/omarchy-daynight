//! Installing and removing the systemd user units.

use std::process::Command;

use crate::config::{Config, PROGRAM_NAME, home, unit_dir};
use crate::theme::omarchy_path;

pub const SERVICE_NAME: &str = "omarchy-daynight.service";
pub const TIMER_NAME: &str = "omarchy-daynight.timer";

/// The legacy fixed-time units this project replaces.
const LEGACY_UNITS: [&str; 2] = ["theme-day.timer", "theme-night.timer"];

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn service_unit() -> String {
    format!(
        r#"[Unit]
Description=Omarchy theme switch driven by real sunrise and sunset
Documentation=https://github.com/windy664/{PROGRAM_NAME}
After=default.target

[Service]
Type=oneshot
# The systemd user manager does not have OMARCHY_PATH when it starts, and a
# catch-up run happens exactly then. Without this the theme lookup fails with
# "Theme '...' does not exist".
Environment=OMARCHY_PATH={omarchy_path}
ExecStart={binary} run
"#,
        omarchy_path = omarchy_path(),
        binary = binary_path().display(),
    )
}

fn timer_unit(poll_interval: &str) -> String {
    format!(
        r#"[Unit]
Description=Check the sun phase and switch the Omarchy theme

[Timer]
OnBootSec=3min
OnUnitActiveSec={poll_interval}
AccuracySec=1min
Unit={SERVICE_NAME}

[Install]
WantedBy=timers.target
"#
    )
}

/// Where the tool is installed.
pub fn binary_path() -> std::path::PathBuf {
    home().join(".local/bin").join(PROGRAM_NAME)
}

fn write_units(config: &Config) -> Result<(), String> {
    let dir = unit_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("could not create {dir:?}: {error}"))?;

    let service = dir.join(SERVICE_NAME);
    let timer = dir.join(TIMER_NAME);
    std::fs::write(&service, service_unit()).map_err(|error| format!("{service:?}: {error}"))?;
    std::fs::write(&timer, timer_unit(&config.poll_interval))
        .map_err(|error| format!("{timer:?}: {error}"))?;
    Ok(())
}

/// Disable the fixed-time units this project replaces.
fn migrate_legacy() -> Vec<String> {
    let mut disabled = Vec::new();
    for unit in LEGACY_UNITS {
        let listed = Command::new("systemctl")
            .args(["--user", "list-unit-files", unit])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);

        if listed && systemctl(&["is-enabled", unit]) && systemctl(&["disable", "--now", unit]) {
            disabled.push(unit.to_string());
        }
    }
    disabled
}

/// Install the units and enable the timer.
pub fn install(config: &mut Config, migrate: bool) -> Result<(), String> {
    write_units(config)?;

    if !systemctl(&["daemon-reload"]) {
        return Err("could not reload the systemd user manager".to_string());
    }
    if !systemctl(&["enable", "--now", TIMER_NAME]) {
        return Err(format!("could not enable {TIMER_NAME}"));
    }

    if migrate {
        for unit in migrate_legacy() {
            println!("{PROGRAM_NAME}: disabled the legacy {unit}");
        }
    }

    Ok(())
}

/// Remove the units and stop the timer.
pub fn uninstall() -> Result<(), String> {
    systemctl(&["disable", "--now", TIMER_NAME]);

    let dir = unit_dir();
    for name in [SERVICE_NAME, TIMER_NAME] {
        let path = dir.join(name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
    systemctl(&["daemon-reload"]);

    Ok(())
}

/// Whether the timer is currently running.
pub fn timer_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", TIMER_NAME])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim() == "active")
        .unwrap_or(false)
}
