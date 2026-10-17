#!/usr/bin/env bash
# Builds the packaged HL2RS executable and runs a bounded main-menu session:
# the game exits on its own after --frames, and a watchdog kills it after 180 s
# so the shell never hangs. Exit status is the game's status (or 124 on timeout).
set -euo pipefail
cd "$(dirname "$0")/../.."

bash scripts/build-bevy.sh

LOG="artifacts/runtime_check.log"
mkdir -p artifacts
./bin/hl2-bevy --no-setup --frames 300 --windowed --no-focus >"$LOG" 2>&1 &
PID=$!
( sleep 180; kill -9 "$PID" 2>/dev/null && echo "watchdog: killed after 180 s" >>"$LOG" ) &
WATCHDOG=$!
set +e
wait "$PID"
STATUS=$?
set -e
kill "$WATCHDOG" 2>/dev/null || true
echo "=== exit status: $STATUS ==="
grep -iE "panic|error|warn" "$LOG" | head -40 || true
tail -n 15 "$LOG"
exit "$STATUS"
