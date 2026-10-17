#!/usr/bin/env bash
# Builds HL2RS and replays main-menu submenu navigation from bevy-menu-submenus.json (Episode One, back,
# Half-Life 2, back, Extras, Lost Coast) over the background level, then prints which
# <game>/<map> backgrounds loaded and the final campaign state from the run report
# (show_report.py). The user's bin/hl2-config.json is backed up and restored (the background
# choice is persisted).
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
LOG="artifacts/runtime_submenus.log"
BACKUP="artifacts/hl2-config.submenus.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
RUST_LOG="info" ./bin/hl2-bevy --no-setup --map background01 --main-menu --windowed --frames 2400 \
  --movement-script test-inputs/bevy-menu-bevy-menu-submenus.json >"$LOG" 2>&1 &
PID=$!
( sleep 240; kill -9 "$PID" 2>/dev/null ) &
WATCHDOG=$!
set +e
wait "$PID"
STATUS=$?
set -e
kill "$WATCHDOG" 2>/dev/null || true
wait "$WATCHDOG" 2>/dev/null || true
echo "=== exit status: $STATUS ==="
sed -E 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -iE "background|failed|panic|warn" | grep -v "Skipped event" | cut -c1-200 | head -40
echo "=== loaded backgrounds and final campaign state ==="
python3 scripts/checks/show_report.py
exit "$STATUS"
