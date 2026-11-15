#!/usr/bin/env bash
# Run the test suite.

set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."
exec python3 test/solar_test.py
