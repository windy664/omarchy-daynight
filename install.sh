#!/usr/bin/env bash
# Build and install omarchy-daynight, then enable the timer.
#
#   ./install.sh              install and enable the timer
#   ./install.sh --migrate-legacy
#                            also disable the fixed-time theme-day/theme-night
#                            timers that this project replaces

set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required: https://rustup.rs" >&2
  exit 1
fi

echo "Building omarchy-daynight..."
cargo install --path . --root "${HOME}/.local" --locked

exec "${HOME}/.local/bin/omarchy-daynight" install "$@"
