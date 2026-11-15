//! Talking to Omarchy: reading the current theme, switching it, and the
//! optional night light.

use std::process::{Command, Stdio};

use crate::config::current_theme_file;

/// The value of OMARCHY_PATH.
///
/// The systemd user manager does not have it at startup, which is exactly when
/// a catch-up run happens, so the tool sets it for every child process it
/// starts.
pub fn omarchy_path() -> String {
    if let Ok(value) = std::env::var("OMARCHY_PATH")
        && !value.is_empty()
    {
        return value;
    }

    // /etc/omarchy.conf is written by omarchy-dev-link and removed again by
    // omarchy-dev-unlink, and it is the only place the development path is
    // recorded.
    if let Ok(text) = std::fs::read_to_string("/etc/omarchy.conf") {
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("OMARCHY_PATH=") {
                let value = rest.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return value.to_string();
                }
            }
        }
    }

    "/usr/share/omarchy".to_string()
}

fn omarchy_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env("OMARCHY_PATH", omarchy_path());
    command
}

/// The theme Omarchy is using right now.
pub fn current_theme() -> Option<String> {
    let text = std::fs::read_to_string(current_theme_file()).ok()?;
    let name = text.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Switch to another theme. Every Omarchy theme hook runs as usual.
pub fn switch_theme(theme: &str) -> Result<(), String> {
    let status = omarchy_command("omarchy")
        .args(["theme", "set", theme])
        .stdin(Stdio::inherit())
        .status()
        .map_err(|error| format!("could not run omarchy: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("omarchy theme set {theme} failed"))
    }
}

/// Whether the night light is currently on.
pub fn nightlight_enabled() -> bool {
    let output = omarchy_command("omarchy-toggle-nightlight")
        .arg("--status")
        .output();

    let Ok(output) = output else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    // The status is JSON: {"enabled":true,"temperature":4000}
    text.contains("\"enabled\":true")
}

/// Turn the night light on or off.
pub fn set_nightlight(on: bool) {
    if nightlight_enabled() == on {
        return;
    }
    let _ = omarchy_command("omarchy-toggle-nightlight").status();
}

/// Show a desktop notification, if the machine has one.
pub fn notify(title: &str, body: &str) {
    let _ = omarchy_command("omarchy-notification-send")
        .args(["-u", "low", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
