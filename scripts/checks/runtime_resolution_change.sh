#!/usr/bin/env bash
# Builds HL2RS, opens the main menu windowed at 1024x768, changes the resolution with
# `mat_setvideomode 1280 720 1`, then opens Options from the keyboard
# (test-inputs/bevy-menu-resolution-change.json) and captures the window to
# artifacts/resolution_change.png: the Options dialog must be visible at 1280x720 (the picture
# must not stay on the frame shown before the resize). Prints the [video] log and the final
# console mode. Backs up/restores bin/hl2-config.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
LOG="artifacts/runtime_resolution_change.log"
BACKUP="artifacts/hl2-config.resolution_change.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
rm -f artifacts/resolution_change.png
RUST_LOG="info,hl2_bevy=debug" ./bin/hl2-bevy --no-setup --map background01 --main-menu --windowed \
  --width 1024 --height 768 --frames 600 --no-focus \
  --movement-script test-inputs/bevy-menu-resolution-change.json \
  --capture artifacts/resolution_change.png >"$LOG" 2>&1 &
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
sed -E 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -E "\[video\] (apply|frame resize|resizing)|panic|ERROR" | sed -E 's/^.*hl2_bevy[^ ]* //' | cut -c1-160
python3 -c 'import json;print("console mode:", json.load(open("artifacts/bevy-report.json"))["render"]["presentation"]["console"]["mode"])'
ls -la artifacts/resolution_change.png 2>/dev/null || echo "no capture"
exit "$STATUS"
