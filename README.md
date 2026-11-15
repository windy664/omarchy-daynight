# omarchy-daynight

Switch the Omarchy theme at **real sunrise and sunset** instead of at a fixed
hour.

Omarchy has no built-in way to follow the sun. `hyprsunset` only shifts the
screen colour temperature at a time you write down by hand, and the theme
picker is manual. This project adds a timer that computes today's sunrise and
sunset for your coordinates and moves between a day theme and a night theme
on its own.

- **One static binary.** Written in Rust. No interpreter, no Python, no
  runtime dependencies to install.
- **No network at runtime.** The sun's times are computed locally.
- **Accurate.** Within 25 seconds of the sunrise-sunset.org API across five
  cities and four seasons.
- **Self-healing.** Miss a sunrise because the laptop was asleep? The next
  check puts the theme right.
- **China friendly.** `locate` finds your coordinates through the China
  Meteorological Administration, which needs no API key and no VPN.

---

## Why this exists

Most people write their own version of this: two `systemd --user` timers,
`07:00` to a light theme and `19:00` to a dark one. It works on the first
day. Then it quietly stops working, and the reason is worth knowing.

### The bug that breaks every hand-written version

```
11月 15 09:41:10.169 omarchy systemd[902]: Starting 白天主题：Catppuccin Latte...
11月 15 09:41:10.199 omarchy omarchy[914]: Theme 'catppuccin-latte' does not exist
11月 15 09:41:10.199 omarchy systemd[902]: theme-day.service: Main process exited, code=exited, status=1/FAILURE
```

`omarchy-theme-set` resolves a theme name like this:

```bash
OMARCHY_THEMES_PATH="$OMARCHY_PATH/themes"
if [[ ! -d $OMARCHY_THEMES_PATH/$THEME_NAME ]] && [[ ! -d $USER_THEMES_PATH/$THEME_NAME ]]; then
  echo "Theme '$THEME_NAME' does not exist"
  exit 1
fi
```

`$OMARCHY_PATH` is not in the systemd **user manager**'s environment when the
manager starts. It is injected a moment later, by uwsm's environment
preloader, as the graphical session comes up:

```
09:41:10.121  systemd[902]: Started 每天 07:00 切亮色主题     <- timer activated
09:41:10.169  systemd[902]: Starting 白天主题                  <- the run
09:41:10.199  omarchy[914]: Theme 'catppuccin-latte' does not exist
09:41:10.668  uwsm[941]: Selected compositor ID: hyprland.desktop <- env arrives, 500ms late
```

A timer with `Persistent=true` catches up a missed calendar event the moment
it is activated, which is exactly at manager startup. So the catch-up always
loses the race, `$OMARCHY_PATH` is still empty, the path becomes `/themes`,
and the theme is reported as missing.

Two things follow:

1. A run on time (07:00:00 with the machine awake) works fine, because uwsm
   imported the environment long before.
2. A run after a boot or a resume fails every time.

That is why the failure looks random: it only happens on the days the
machine was off at the switch hour.

The fix is one line in the unit file:

```ini
[Service]
Environment=OMARCHY_PATH=/usr/share/omarchy
```

This project ships that line in its unit, and also sets it for every process
it starts, so it works however it is launched.

---

## Install

```bash
git clone https://github.com/windy664/omarchy-daynight
cd omarchy-daynight
./install.sh
```

`install.sh` builds the binary with cargo, installs it to
`~/.local/bin/omarchy-daynight`, writes the config, drops the systemd user
units in place and enables the timer. It looks up your coordinates on the
way, if it can.

Already have fixed-time `theme-day.timer` / `theme-night.timer` units? Turn
them off in the same step:

```bash
./install.sh --migrate-legacy
```

### Requirements

- Omarchy (the theme switching goes through `omarchy theme set`)
- A Rust toolchain, if you are building from source (1.85 or newer, for the
  2024 edition)
- `systemd --user` with timers, which Omarchy already provides

Nothing else. The timer needs no root.

### Building from a mirror

The crates.io index can be slow from mainland China. Point cargo at a mirror
in `~/.cargo/config.toml`:

```toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
```

---

## Configuration

`~/.config/omarchy-daynight/config`

```ini
# The theme applied between sunrise and sunset, and between sunset and sunrise.
day_theme=catppuccin-latte
night_theme=catppuccin

# Where you are. `install` fills these in for you.
latitude=22.97
longitude=113.74
# Empty means: take the timezone from the system.
timezone=Asia/Shanghai

# Shift the switch by minutes: negative switches earlier, positive later.
dawn_offset=0
dusk_offset=0

# How often the timer checks whether the phase changed.
poll_interval=10min

# Used only inside the polar circles, where the sun may never rise or set.
fallback_day_start=07:00
fallback_night_start=19:00

# Night light (hyprsunset) handling: off, auto.
nightlight=off

# Optional command run after every theme change. It has to be quoted, since
# the config file is read by sourcing it.
on_switch="sleep 3 && $HOME/.local/bin/random-wallpaper"
```

Any theme Omarchy knows works. List them with `omarchy theme list`. The
Catppuccin pair (`catppuccin-latte` and `catppuccin`) ships with Omarchy;
`flexoki-light` and `tokyo-night` are good alternatives.

### Offsets

`dawn_offset` and `dusk_offset` shift the switch away from the exact moment
the sun crosses the horizon. `dawn_offset=-15` switches to the light theme a
quarter of an hour before sunrise, which often feels better than switching
at sunrise, when the screen is still dim.

### Pause it

```bash
touch ~/.local/state/omarchy-daynight/pause   # stop switching
rm ~/.local/state/omarchy-daynight/pause       # start again
```

---

## Commands

```bash
omarchy-daynight status     # today: sunrise, sunset, current and wanted theme
omarchy-daynight run        # switch now if the phase changed
omarchy-daynight locate     # guess your coordinates from your public IP
omarchy-daynight install    # install and enable
omarchy-daynight uninstall  # remove the units, keep the config
omarchy-daynight selftest   # check the solar calculation
```

`status` output:

```
location   22.97, 113.74 (Asia/Shanghai)
sunrise    2026-11-15T06:38:14+0800
sunset     2026-11-15T17:40:49+0800
phase      morning
theme now  catppuccin-latte
theme want catppuccin-latte
timer      active
```

---

## How it works

Sunrise and sunset drift by a few minutes every day, so a calendar timer
cannot hold a fixed hour and stay right. This project polls instead:

1. A timer wakes up every `poll_interval` (ten minutes by default).
2. The tool computes today's sunrise and sunset for your coordinates.
3. It works out whether it is currently the day half or the night half.
4. If that differs from the theme Omarchy is using, it switches, and nothing
   happens if it does not.

Polling also means a missed sunrise is picked up on the next check, rather
than being lost. The theme is compared before every switch, so the tool
does nothing when it is already right.

The calculation uses the NOAA solar position algorithm, evaluated at the
sun's transit rather than at local midnight. That detail matters: taking the
declination at midnight is two minutes out on the day length at high
latitude, and the error grows towards the equinoxes.

Polar day and polar night are detected and fall back to
`fallback_day_start` / `fallback_night_start`.

### Accuracy

Measured against the sunrise-sunset.org API, 22 cases across five cities
(Dongguan, Beijing, New York, Sydney, Reykjavik) and four seasons:

| | worst deviation |
|---|---|
| sunrise | 0.36 min |
| sunset | 0.36 min |

The test suite also checks the calculation against `astral`, an unrelated
implementation of the same algorithm, and against day lengths taken from
the API:

```bash
cargo test
omarchy-daynight selftest
```

`cargo test` runs the unit tests, `selftest` runs the same reference checks
inside the installed binary.

---

## Finding your coordinates

`omarchy-daynight locate` uses two services that work from mainland China
without a VPN and without an API key:

1. `myip.ipip.net` reports your city.
2. `weather.cma.cn` (China Meteorological Administration) turns that city
   into a station, and the station carries the latitude, longitude and
   timezone.

Set `auto_locate=1` to refresh the coordinates on every run, or just fill in
`latitude` and `longitude` by hand and keep it that way. The sun does not
move much between neighbouring towns.

If you are outside China, skip this and set the coordinates yourself, or use
any geocoder you like.

---

## Night light

Omarchy's `hyprsunset` is off by default because it tints the screen. Set
`nightlight=auto` to have this tool turn it on at sunset and off at
sunrise, using the same `omarchy toggle nightlight` switch the menu uses.

---

## Uninstall

```bash
omarchy-daynight uninstall
```

That stops the timer and removes the units. The config file is left alone.

---

## Notes

- The theme switch goes through `omarchy theme set`, so every Omarchy theme
  hook runs as usual: the GTK theme follows, the browser policy follows,
  terminals and editors follow.
- `on_switch` runs a command after every theme change. Use it to rotate the
  wallpaper, for example.
- The units are user units, so they need no root and survive a reboot.
- The crates used are `chrono`, `chrono-tz`, `ureq` and `serde`. Everything
  else is the standard library.

## Licence

MIT. See [LICENSE](LICENSE).
