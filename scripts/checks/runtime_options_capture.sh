#!/usr/bin/env bash
# Replays every options_capture*.json at 1024x768 (the UI canvas is the render resolution, so
# scripted pointer positions are 1024x768 canvas pixels; main menu: 5x Down, Enter opens
# Options, then scripted pointer clicks) over hl2/background01 with the packaged
# bin/hl2-bevy as it is (no rebuild). Each <name>.json is captured at the end to
# artifacts/<name>.png (bevy-options-capture.json: Keyboard > Advanced;
# bevy-options-capture-video-advanced.json: Video > Advanced). bin/hl2-config.json is backed up and
# restored.
set -euo pipefail
cd "$(dirname "$0")/../.."
mkdir -p artifacts
BACKUP="artifacts/hl2-config.options_capture.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
OVERALL=0
for SCRIPT in test-inputs/bevy-options-capture*.json; do
  NAME="$(basename "$SCRIPT" .json)"
  LOG="artifacts/runtime_$NAME.log"
  ./bin/hl2-bevy --no-setup --map background01 --main-menu --background --width 1024 --height 768 --frames 400 \
    --movement-script "$SCRIPT" --capture "artifacts/$NAME.png" >"$LOG" 2>&1 &
  PID=$!
  ( sleep 180; kill -9 "$PID" 2>/dev/null ) &
  WATCHDOG=$!
  set +e
  wait "$PID"
  STATUS=$?
  set -e
  # The sleep would otherwise outlive its subshell and hold the output open.
  pkill -P "$WATCHDOG" 2>/dev/null || true
  kill "$WATCHDOG" 2>/dev/null || true
  echo "=== $NAME exit status: $STATUS ==="
  grep -iE "panic|error" "$LOG" | head -5 || true
  [ "$STATUS" -ne 0 ] && OVERALL="$STATUS"
done
exit "$OVERALL"
