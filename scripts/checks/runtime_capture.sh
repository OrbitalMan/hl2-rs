#!/usr/bin/env bash
# Builds HL2RS and captures the window after a bounded windowed main-menu session to
# artifacts/present_check.png (checks that the presented frame reaches the window).
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
LOG="artifacts/runtime_capture.log"
./bin/hl2-bevy --no-setup --frames 240 --windowed --no-focus --capture artifacts/present_check.png >"$LOG" 2>&1 &
PID=$!
( sleep 180; kill -9 "$PID" 2>/dev/null ) &
WATCHDOG=$!
set +e
wait "$PID"
STATUS=$?
set -e
kill "$WATCHDOG" 2>/dev/null || true
echo "=== exit status: $STATUS ==="
ls -la artifacts/present_check.png
exit "$STATUS"
