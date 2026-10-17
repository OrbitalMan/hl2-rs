#!/usr/bin/env bash
# Builds HL2RS and replays bevy-menu-return.json: Episode Two submenu, then the Half-Life 2 map
# d1_trainstation_02 from the console, then Pause > Return to main menu (confirmed with Enter).
# Prints the loaded <game>/<map> sequence and final campaign state (show_report.py); the
# menu must come back over hl2/background01. Backs up/restores bin/hl2-config.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
LOG="artifacts/runtime_return_menu.log"
BACKUP="artifacts/hl2-config.return_menu.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
RUST_LOG="info" ./bin/hl2-bevy --no-setup --map background01 --main-menu --background --frames 3200 \
  --movement-script test-inputs/bevy-menu-return.json >"$LOG" 2>&1 &
PID=$!
( sleep 240; kill -9 "$PID" 2>/dev/null ) &
WATCHDOG=$!
set +e
wait "$PID"
STATUS=$?
set -e
# The sleep would otherwise outlive its subshell and hold the output open.
pkill -P "$WATCHDOG" 2>/dev/null || true
kill "$WATCHDOG" 2>/dev/null || true
wait "$WATCHDOG" 2>/dev/null || true
echo "=== exit status: $STATUS ==="
sed -E 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -iE "background|failed|panic|warn" | grep -v "Skipped event" | cut -c1-200 | head -40
echo "=== loaded backgrounds and final campaign state ==="
python3 scripts/checks/show_report.py
exit "$STATUS"
