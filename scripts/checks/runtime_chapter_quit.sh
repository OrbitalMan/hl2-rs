#!/usr/bin/env bash
# Builds HL2RS, starts the gameplay map d2_coast_01 (Half-Life 2 chapter 7) and exits after
# 600 frames (the saved background is first set to hl2/background01) without returning to the menu, then prints the saved menu background from
# bin/hl2-config.json; it must read hl2/background04. Backs up/restores the config.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
LOG="artifacts/runtime_chapter_quit.log"
BACKUP="artifacts/hl2-config.chapter_quit.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
BG_GAME=hl2 BG_MAP=background01 python3 scripts/checks/set_menu_background.py
echo "=== saved menu background before ==="
grep -E '"background_(game|map)"' bin/hl2-config.json
RUST_LOG="info" ./bin/hl2-bevy --no-setup --map d2_coast_01 --windowed --frames 600 >"$LOG" 2>&1 &
PID=$!
( sleep 180; kill -9 "$PID" 2>/dev/null ) &
WATCHDOG=$!
set +e
wait "$PID"
STATUS=$?
set -e
kill "$WATCHDOG" 2>/dev/null || true
wait "$WATCHDOG" 2>/dev/null || true
echo "=== exit status: $STATUS ==="
sed -E 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -iE "panic|error" | cut -c1-200 | head -10 || true
echo "=== saved menu background ==="
grep -E '"background_(game|map)"' bin/hl2-config.json
exit "$STATUS"
