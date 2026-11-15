#!/usr/bin/env bash
# Convenience wrapper: install omarchy-sun-theme from a checkout.
#
#   ./install.sh              install and enable the timer
#   ./install.sh --migrate-legacy
#                            also disable the fixed-time theme-day/theme-night
#                            timers that this project replaces

set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"
exec ./bin/omarchy-sun-theme install "$@"
