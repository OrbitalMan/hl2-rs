#!/usr/bin/env bash
# Rebuilds bin/hl2-bevy, sets the menu background to episodic/ep1_background02 (Kleiner's
# broadcast) and runs a bounded 1024x768 windowed session (180 s watchdog) twice: with closed
# captions on (capture artifacts/captions_capture.png) and off (captions_off.png),
# printing the caption report of each. Backs up/restores bin/hl2-config.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
BACKUP="artifacts/hl2-config.captions.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
for pass in closed_captions:captions_capture none:captions_off; do
  CAPTIONS="${pass%%:*}" python3 scripts/checks/set_captions_config.py
  OUT="artifacts/${pass##*:}.png"
  rm -f "$OUT"
  ./bin/hl2-bevy --no-setup --frames 420 --windowed --width 1024 --height 768 --no-focus --capture "$OUT" \
    >"artifacts/runtime_${pass##*:}.log" 2>&1 &
  PID=$!
  ( sleep 180; kill -9 "$PID" 2>/dev/null ) &
  WATCHDOG=$!
  set +e
  wait "$PID"
  STATUS=$?
  set -e
  kill "$WATCHDOG" 2>/dev/null || true
  echo "=== captions ${pass%%:*}: exit status $STATUS ==="
  python3 scripts/checks/show_report_captions.py
  ls -la "$OUT" 2>/dev/null || echo "no capture"
done
