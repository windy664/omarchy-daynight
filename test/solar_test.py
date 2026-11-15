#!/usr/bin/env python3
"""Reference tests for the solar calculation.

The expected values come from `astral`, an independent implementation of the
same NOAA algorithm, so the test compares two implementations rather than the
code against itself. Run it with no dependencies:

    python3 test/solar_test.py
"""

from __future__ import annotations

import os
import sys
from datetime import date, datetime

_here = os.path.dirname(os.path.abspath(__file__))
for _candidate in (os.path.join(_here, "..", "lib"), _here):
    if os.path.isfile(os.path.join(_candidate, "solar.py")):
        sys.path.insert(0, _candidate)
        break

import solar  # noqa: E402

# Reference values produced by `astral`, an independent implementation of the
# same NOAA algorithm, so the test compares two implementations rather than
# the code against itself.
REFERENCE = [
    ("Dongguan", "2026-01-15", "07:08", "18:00", 22.97, 113.74, "Asia/Shanghai", 2),
    ("Dongguan", "2026-04-15", "06:05", "18:45", 22.97, 113.74, "Asia/Shanghai", 2),
    ("Dongguan", "2026-07-15", "05:48", "19:13", 22.97, 113.74, "Asia/Shanghai", 2),
    ("Dongguan", "2026-10-15", "06:21", "17:59", 22.97, 113.74, "Asia/Shanghai", 2),
    ("Beijing", "2026-01-15", "07:34", "17:13", 39.90, 116.40, "Asia/Shanghai", 2),
    ("Beijing", "2026-04-15", "05:37", "18:52", 39.90, 116.40, "Asia/Shanghai", 2),
    ("Beijing", "2026-07-15", "04:58", "19:41", 39.90, 116.40, "Asia/Shanghai", 2),
    ("Beijing", "2026-10-15", "06:24", "17:35", 39.90, 116.40, "Asia/Shanghai", 2),
    ("Urumqi", "2026-01-15", "09:40", "18:57", 43.83, 87.62, "Asia/Shanghai", 2),
    ("Urumqi", "2026-04-15", "07:27", "20:52", 43.83, 87.62, "Asia/Shanghai", 2),
    ("Urumqi", "2026-07-15", "06:41", "21:49", 43.83, 87.62, "Asia/Shanghai", 2),
    ("Urumqi", "2026-10-15", "08:23", "19:26", 43.83, 87.62, "Asia/Shanghai", 2),
    ("Sydney", "2026-01-15", "05:59", "20:08", -33.87, 151.21, "Australia/Sydney", 2),
    ("Sydney", "2026-04-15", "06:17", "17:32", -33.87, 151.21, "Australia/Sydney", 2),
    ("Sydney", "2026-07-15", "06:58", "17:04", -33.87, 151.21, "Australia/Sydney", 2),
    ("Sydney", "2026-10-15", "06:14", "19:08", -33.87, 151.21, "Australia/Sydney", 2),
    ("NewYork", "2026-01-15", "07:18", "16:53", 40.71, -74.01, "America/New_York", 2),
    ("NewYork", "2026-04-15", "06:17", "19:35", 40.71, -74.01, "America/New_York", 2),
    ("NewYork", "2026-07-15", "05:38", "20:25", 40.71, -74.01, "America/New_York", 2),
    ("NewYork", "2026-10-15", "07:07", "18:15", 40.71, -74.01, "America/New_York", 2),
    # High latitude: the sun's path is shallow, so the same algorithm drifts
    # a little more here.
    ("Reykjavik", "2026-01-15", "10:55", "16:19", 64.15, -21.94, "Atlantic/Reykjavik", 3),
    ("Reykjavik", "2026-04-15", "05:56", "21:01", 64.15, -21.94, "Atlantic/Reykjavik", 3),
    ("Reykjavik", "2026-10-15", "08:18", "18:07", 64.15, -21.94, "Atlantic/Reykjavik", 3),
]

# Day lengths in minutes, taken from the sunrise-sunset.org API, which is an
# independent implementation again. The declination has to be evaluated at the
# transit rather than at local midnight, or the day length drifts by up to
# three minutes at high latitude; these values pin that down.
DAY_LENGTHS = [
    ("2026-10-15", 590, 64.15, -21.94, "Atlantic/Reykjavik"),
    ("2026-01-15", 326, 64.15, -21.94, "Atlantic/Reykjavik"),
    ("2026-07-15", 1184, 64.15, -21.94, "Atlantic/Reykjavik"),
    ("2026-10-15", 669, 40.71, -74.01, "America/New_York"),
    ("2026-01-15", 576, 40.71, -74.01, "America/New_York"),
    ("2026-10-15", 698, 22.97, 113.74, "Asia/Shanghai"),
    ("2026-01-15", 653, 22.97, 113.74, "Asia/Shanghai"),
    ("2026-10-15", 671, 39.90, 116.40, "Asia/Shanghai"),
]

failures = []
checks = 0


def check(condition: bool, message: str) -> None:
    global checks
    checks += 1
    if not condition:
        failures.append(message)


def minutes_from_midnight(stamp: datetime) -> float:
    return stamp.hour * 60 + stamp.minute + stamp.second / 60.0


def parse_hhmm(text: str) -> float:
    hours, minutes = text.split(":")
    return int(hours) * 60 + int(minutes)


def test_reference_values() -> None:
    worst = 0.0
    for place, day, rise, setting, lat, lon, tz, tolerance in REFERENCE:
        events = solar.solar_events(date.fromisoformat(day), lat, lon, tz)
        for label, expected_text, actual in (
            ("sunrise", rise, events.sunrise),
            ("sunset", setting, events.sunset),
        ):
            drift = abs(minutes_from_midnight(actual) - parse_hhmm(expected_text))
            worst = max(worst, drift)
            check(
                drift <= tolerance,
                f"{place} {day} {label}: expected {expected_text}, got "
                f"{actual:%H:%M} ({drift:.1f} min off, tolerance {tolerance})",
            )
    print(f"  reference values: {len(REFERENCE)} days checked, worst drift {worst:.2f} min")


def test_polar_day() -> None:
    # Tromso sits well inside the Arctic Circle: the sun does not set in June.
    events = solar.solar_events(date(2026, 6, 21), 69.65, 18.96, "Europe/Oslo")
    check(
        events.kind == "midnight_sun",
        f"expected midnight_sun in Tromso in June, got {events.kind}",
    )
    check(events.sunrise is None, "midnight sun should have no sunrise")
    check(events.day_length_minutes == 1440, "midnight sun should report a full day")
    print("  polar day: Tromso in June reports midnight_sun")


def test_polar_night() -> None:
    # The sun does not rise at the north pole in December.
    events = solar.solar_events(date(2026, 12, 15), 89.0, 0.0, "UTC")
    check(
        events.kind == "polar_night",
        f"expected polar_night at the north pole in December, got {events.kind}",
    )
    check(events.day_length_minutes == 0, "polar night should report no daylight")
    print("  polar night: the north pole in December reports polar_night")


def test_day_lengths() -> None:
    for day, expected, lat, lon, tz in DAY_LENGTHS:
        events = solar.solar_events(date.fromisoformat(day), lat, lon, tz)
        drift = abs(events.day_length_minutes - expected)
        check(
            drift <= 1,
            f"{tz} {day} day length: expected {expected} min, got "
            f"{events.day_length_minutes} min",
        )
    print(f"  day lengths: {len(DAY_LENGTHS)} cases checked against the API")


def test_phase() -> None:
    from datetime import timedelta

    events = solar.solar_events(date(2026, 11, 15), 22.97, 113.74, "Asia/Shanghai")

    before = events.sunrise - timedelta(hours=3)
    during = events.noon
    after = events.sunset + timedelta(hours=3)

    check(solar.phase(before, events) == solar.EVENING, "before sunrise should be evening")
    check(solar.phase(during, events) == solar.MORNING, "midday should be morning")
    check(solar.phase(after, events) == solar.EVENING, "after sunset should be evening")

    # A dawn offset moves the switch: half an hour after sunrise is still
    # evening with a +60 minute offset, and morning 90 minutes after.
    half_past = events.sunrise + timedelta(minutes=30)
    late = events.sunrise + timedelta(minutes=90)
    check(
        solar.phase(half_past, events) == solar.MORNING,
        "half an hour after sunrise should be morning",
    )
    check(
        solar.phase(half_past, events, dawn_offset=60) == solar.EVENING,
        "a dawn offset of +60 should not have switched yet at sunrise + 30",
    )
    check(
        solar.phase(late, events, dawn_offset=60) == solar.MORNING,
        "a dawn offset of +60 should have switched by sunrise + 90",
    )
    print("  phase: before/during/after and dawn offsets behave as expected")


def test_daylight_length() -> None:
    events = solar.solar_events(date(2026, 11, 15), 22.97, 113.74, "Asia/Shanghai")
    check(
        600 < events.day_length_minutes < 700,
        f"daylight length looks wrong: {events.day_length_minutes} min",
    )
    print(f"  daylight length: {events.day_length_minutes} min")


def main() -> int:
    print("solar.py self test")
    test_reference_values()
    test_day_lengths()
    test_polar_day()
    test_polar_night()
    test_phase()
    test_daylight_length()

    if failures:
        print(f"\n{len(failures)} of {checks} checks failed:")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(f"\nall {checks} checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
