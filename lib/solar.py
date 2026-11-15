#!/usr/bin/env python3
"""Solar events (sunrise, sunset, solar noon) computed locally.

Standard library only -- no pip install, no network access. Uses the NOAA
solar position algorithm (the one behind the NOAA Sunrise/Sunset calculator
spreadsheet), which is accurate to well under a minute for the latitudes
people actually live at.

The same numbers Omarchy's own "elsewhen" panel draws are produced by
Sun.js; this module exists so the project works on any machine, with or
without that plugin.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from typing import Optional

try:
    from zoneinfo import ZoneInfo
except ImportError:  # pragma: no cover - Python < 3.9
    ZoneInfo = None  # type: ignore[assignment]

# Official sunrise: geometric zenith of the sun (90 deg) plus atmospheric
# refraction near the horizon and the sun's own radius.
SUNRISE_ZENITH_DEG = 90.833

MORNING = "morning"
EVENING = "evening"


class SolarError(Exception):
    """Raised when the requested location or timezone cannot be resolved."""


@dataclass
class SolarEvents:
    """Sun events for one calendar day at one place."""

    day: date
    sunrise: Optional[datetime]
    sunset: Optional[datetime]
    noon: Optional[datetime]
    day_length_minutes: Optional[int]
    # "normal", "midnight_sun" (sun never sets) or "polar_night" (never rises)
    kind: str

    def as_dict(self) -> dict:
        return {
            "date": self.day.isoformat(),
            "sunrise": self.sunrise.isoformat() if self.sunrise else None,
            "sunset": self.sunset.isoformat() if self.sunset else None,
            "noon": self.noon.isoformat() if self.noon else None,
            "day_length_minutes": self.day_length_minutes,
            "kind": self.kind,
        }


def _zone(name: str):
    if ZoneInfo is None:
        raise SolarError("Python 3.9 or newer is required for timezone support")
    try:
        return ZoneInfo(name)
    except Exception as exc:  # noqa: BLE001 - ZoneInfo raises several types
        raise SolarError(f"unknown timezone {name!r}: {exc}") from exc


def _julian_day(day: date) -> float:
    """Julian day at 00:00 UT of the given calendar date."""
    year, month = day.year, day.month
    if month <= 2:
        year -= 1
        month += 12
    a = year // 100
    b = 2 - a + a // 4
    return (
        math.floor(365.25 * (year + 4716))
        + math.floor(30.6001 * (month + 1))
        + day.day
        + b
        - 1524.5
    )


def _solar_parameters(jd: float) -> tuple[float, float]:
    """Return (declination in degrees, equation of time in minutes)."""
    t = (jd - 2451545.0) / 36525.0

    # Geometric mean longitude and anomaly of the sun.
    mean_long = (280.46646 + t * (36000.76983 + t * 0.0003032)) % 360.0
    mean_anom = 357.52911 + t * (35999.05029 - 0.0001537 * t)

    # Eccentricity of earth's orbit.
    ecc = 0.016708634 - t * (0.000042037 + 0.0000001267 * t)

    # Equation of the centre.
    centre = (
        math.sin(math.radians(mean_anom)) * (1.914602 - t * (0.004817 + 0.000014 * t))
        + math.sin(math.radians(2 * mean_anom)) * (0.019993 - 0.000101 * t)
        + math.sin(math.radians(3 * mean_anom)) * 0.000289
    )

    true_long = mean_long + centre
    omega = 125.04 - 1934.136 * t
    app_long = true_long - 0.00569 - 0.00478 * math.sin(math.radians(omega))

    # Obliquity of the ecliptic, corrected.
    mean_obliq = 23.0 + (
        26.0 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.0
    ) / 60.0
    obliq = mean_obliq + 0.00256 * math.cos(math.radians(omega))

    declination = math.degrees(
        math.asin(math.sin(math.radians(obliq)) * math.sin(math.radians(app_long)))
    )

    # Equation of time, in minutes.
    y = math.tan(math.radians(obliq / 2.0)) ** 2
    eq_time = 4.0 * math.degrees(
        y * math.sin(2 * math.radians(mean_long))
        - 2 * ecc * math.sin(math.radians(mean_anom))
        + 4 * ecc * y * math.sin(math.radians(mean_anom)) * math.cos(2 * math.radians(mean_long))
        - 0.5 * y * y * math.sin(4 * math.radians(mean_long))
        - 1.25 * ecc * ecc * math.sin(2 * math.radians(mean_anom))
    )

    return declination, eq_time


def solar_events(day: date, latitude: float, longitude: float, tz_name: str) -> SolarEvents:
    """Sunrise, sunset and solar noon for a calendar day at a place.

    ``latitude`` is north positive, ``longitude`` east positive. ``tz_name``
    is an IANA zone name such as "Asia/Shanghai".
    """
    zone = _zone(tz_name)
    jd_midnight = _julian_day(day)

    # Offset of the zone from UTC, in minutes, at local noon of that day.
    offset_minutes = _zone_offset_minutes(zone, day)

    # The sun's declination depends on the instant it is evaluated at, and the
    # instant that matters is the transit, which itself depends on the
    # declination. Solving it once at local midnight is good to about two
    # minutes of day length at high latitude, because the declination moves
    # most quickly near the equinoxes. Two rounds around the loop settle it.
    declination, eq_time = _solar_parameters(jd_midnight)
    noon_minutes = 720.0 - 4.0 * longitude - eq_time + offset_minutes
    for _ in range(2):
        jd_noon = jd_midnight + (noon_minutes - offset_minutes) / 1440.0
        declination, eq_time = _solar_parameters(jd_noon)
        noon_minutes = 720.0 - 4.0 * longitude - eq_time + offset_minutes

    cos_ha = (
        math.cos(math.radians(SUNRISE_ZENITH_DEG))
        / (math.cos(math.radians(latitude)) * math.cos(math.radians(declination)))
        - math.tan(math.radians(latitude)) * math.tan(math.radians(declination))
    )

    local_midnight = datetime.combine(day, time.min, tzinfo=zone)

    if cos_ha < -1.0:
        # The sun never sets: midnight sun.
        return SolarEvents(day, None, None, _at(local_midnight, noon_minutes), 1440, "midnight_sun")
    if cos_ha > 1.0:
        # The sun never rises: polar night.
        return SolarEvents(day, None, None, _at(local_midnight, noon_minutes), 0, "polar_night")

    ha_degrees = math.degrees(math.acos(cos_ha))
    sunrise_minutes = noon_minutes - ha_degrees * 4.0
    sunset_minutes = noon_minutes + ha_degrees * 4.0

    sunrise = _at(local_midnight, sunrise_minutes)
    sunset = _at(local_midnight, sunset_minutes)
    noon = _at(local_midnight, noon_minutes)

    return SolarEvents(
        day=day,
        sunrise=sunrise,
        sunset=sunset,
        noon=noon,
        day_length_minutes=int(round(sunset_minutes - sunrise_minutes)),
        kind="normal",
    )


def _at(local_midnight: datetime, minutes: float) -> datetime:
    """Local midnight plus a number of minutes, keeping the local wall clock."""
    return local_midnight + timedelta(minutes=minutes)


def _zone_offset_minutes(zone, day: date) -> int:
    """UTC offset of a zone, in minutes, at local noon of the given day."""
    probe = datetime.combine(day, time(12, 0)).replace(tzinfo=zone)
    return int(probe.utcoffset().total_seconds() // 60)


def phase(now: datetime, events: SolarEvents, dawn_offset: int = 0, dusk_offset: int = 0) -> str:
    """Which half of the day ``now`` falls in: MORNING or EVENING.

    ``dawn_offset`` and ``dusk_offset`` shift sunrise and sunset by minutes,
    so a theme can change a little before the sun actually appears.
    """
    if events.kind != "normal" or events.sunrise is None or events.sunset is None:
        raise SolarError("cannot determine phase for this day")

    sunrise = events.sunrise + timedelta(minutes=dawn_offset)
    sunset = events.sunset + timedelta(minutes=dusk_offset)
    return MORNING if sunrise <= now < sunset else EVENING


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        prog="solar.py", description="Compute sunrise and sunset locally."
    )
    parser.add_argument("--lat", type=float, required=True, help="latitude, north positive")
    parser.add_argument("--lon", type=float, required=True, help="longitude, east positive")
    parser.add_argument("--tz", required=True, help="IANA timezone, e.g. Asia/Shanghai")
    parser.add_argument("--date", help="calendar date, YYYY-MM-DD (default: today)")
    parser.add_argument("--json", action="store_true", help="print JSON")
    parser.add_argument(
        "--now", help="evaluate the phase at this local ISO time (default: now)"
    )
    args = parser.parse_args(argv)

    try:
        zone = _zone(args.tz)
        day = date.fromisoformat(args.date) if args.date else datetime.now(zone).date()
        events = solar_events(day, args.lat, args.lon, args.tz)
    except (SolarError, ValueError) as exc:
        print(f"solar.py: {exc}", file=sys.stderr)
        return 1

    if args.now:
        now = datetime.fromisoformat(args.now)
        if now.tzinfo is None:
            now = now.replace(tzinfo=zone)
    else:
        now = datetime.now(zone)

    payload = events.as_dict()
    try:
        payload["phase"] = phase(now, events)
    except SolarError:
        payload["phase"] = None

    if args.json:
        print(json.dumps(payload, ensure_ascii=False))
    else:
        if events.sunrise:
            print(f"sunrise {events.sunrise:%Y-%m-%d %H:%M:%S %Z}")
        if events.sunset:
            print(f"sunset  {events.sunset:%Y-%m-%d %H:%M:%S %Z}")
        if events.noon:
            print(f"noon    {events.noon:%Y-%m-%d %H:%M:%S %Z}")
        print(f"kind    {events.kind}")
        print(f"phase   {payload['phase']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
