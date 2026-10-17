#!/usr/bin/env bash
# Rebuilds HL2RS and starts the main menu windowed at 2560x1600, then 1024x768 (bounded runs),
# printing the [video] resize/placement lines: on macOS the window must be sized and centred in
# the screen's visible frame (without the menu bar and Dock). Backs up/restores
# bin/hl2-config.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts
BACKUP="artifacts/hl2-config.window_place.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
for size in "2560 1600" "1024 768"; do
  set -- $size
  RUST_LOG="info,hl2_bevy=debug" ./bin/hl2-bevy --no-setup --main-menu --windowed --width "$1" --height "$2" \
    --frames 240 --no-focus >artifacts/runtime_window_place.log 2>&1 || echo "exit $?"
  echo "=== $1x$2 ==="
  sed -E 's/\x1b\[[0-9;]*m//g' artifacts/runtime_window_place.log | grep -E "\[video\] (resize_window|window placed)" | sed -E 's/^.*hl2_bevy[^ ]* //' | cut -c1-170
done
