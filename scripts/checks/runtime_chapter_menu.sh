#!/usr/bin/env bash
# Builds HL2RS and replays bevy-menu-chapter-return.json: the Half-Life 2 maps d2_coast_01 (chapter 7)
# and d3_c17_10a (chapter 11), each loaded from the console and left by Pause > Return to main menu.
# Prints the loaded <game>/<map> sequence and final campaign state (show_report.py); the
# menu must come back over hl2/background04, then hl2/background06 (the chapters' backgrounds).
# Backs up/restores bin/hl2-config.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
LOG="artifacts/runtime_chapter_menu.log"
BACKUP="artifacts/hl2-config.chapter_menu.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
RUST_LOG="info" ./bin/hl2-bevy --no-setup --map background01 --main-menu --windowed --frames 4400 \
  --movement-script test-inputs/bevy-menu-chapter-return.json >"$LOG" 2>&1 &
PID=$!
( sleep 300; kill -9 "$PID" 2>/dev/null ) &
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
