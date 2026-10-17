#!/usr/bin/env bash
# Builds HL2RS and captures each game's chapter 1 menu background at 1024x768 with FOV 75
# (the retail default) to artifacts/bg_compare/<game>_<map>.png, for comparison with the
# retail pre-blurred menu images (the installed materials/console/<map>[_widescreen].vtf,
# decoded with any VTF viewer). Backs up/restores
# bin/hl2-config.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/build-bevy.sh >/dev/null
mkdir -p artifacts/bg_compare
BACKUP="artifacts/hl2-config.bg_compare.backup.json"
[ -f bin/hl2-config.json ] && cp bin/hl2-config.json "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" bin/hl2-config.json; }
trap restore EXIT
for pair in hl2:background01 episodic:ep1_background01 ep2:ep2_background01 lostcoast:background01; do
  BG_GAME="${pair%%:*}" BG_MAP="${pair##*:}" BG_FOV=75 python3 scripts/checks/set_bg_config.py
  OUT="artifacts/bg_compare/${pair%%:*}_${pair##*:}.png"
  rm -f "$OUT"
  ./bin/hl2-bevy --no-setup --frames 300 --background --width 1024 --height 768 --capture "$OUT" \
    >"artifacts/bg_compare/${pair%%:*}.log" 2>&1 || echo "exit $? for $pair"
  python3 scripts/checks/show_report.py | grep -E 'loaded_game|loaded_map' | tr '\n' ' '; echo
  ls -la "$OUT" 2>/dev/null || echo "no capture for $pair"
done
