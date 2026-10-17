#!/usr/bin/env bash
# Checks that the Options dialog's placement persists across runs: run 1 replays
# bevy-options-capture-drag.json (drags the dialog by its caption), prints the saved "dialogs" entry of
# bin/hl2-config.json, then run 2 only opens Options (first 7 entries of bevy-options-capture.json)
# and captures artifacts/placement_persist.png. bin/hl2-config.json is backed up and
# restored. Uses the packaged bin/hl2-bevy as it is (no rebuild).
set -euo pipefail
cd "$(dirname "$0")/../.."
mkdir -p artifacts
BACKUP="artifacts/hl2-config.placement.backup.json"
OPEN="artifacts/placement_open.json"
cp bin/hl2-config.json "$BACKUP"
restore() { cp "$BACKUP" bin/hl2-config.json; rm -f "$OPEN"; }
trap restore EXIT
run() {
  ./bin/hl2-bevy --no-setup --map background01 --main-menu --windowed --width 1024 --height 768 --frames 400 \
    --no-focus --movement-script "$1" --capture "$2" >"artifacts/runtime_placement_persist.log" 2>&1
}
run test-inputs/bevy-options-capture-drag.json artifacts/placement_drag.png
echo "=== saved dialogs ==="
grep -A6 '"dialogs"' bin/hl2-config.json || echo "(none saved)"
{ head -7 test-inputs/bevy-options-capture.json | sed '$ s/,$//'; echo "]"; } > "$OPEN"
run "$OPEN" artifacts/placement_persist.png
echo "=== run 2 captured: artifacts/placement_persist.png ==="
