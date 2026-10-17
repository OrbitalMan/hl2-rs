#!/usr/bin/env bash
# Every packaged replay and capture check, in one script. Runs in the background (`--background`:
# no window, nothing takes focus), builds once, gives each check the user's settings
# (bin/hl2-config.json is backed up once and restored before each check and at the end), checks
# each result and prints one ok/FAIL line per check with its duration. Full output per check:
# artifacts/replays/<check>.log; captures go to artifacts/.
#
# Checks (ONLY="submenus captions" runs a subset; no variable runs all):
#   submenus        Episode One, back, Half-Life 2, back, Extras > Lost Coast: each game's
#                   background loads (test-inputs/bevy-menu-submenus.json)
#   return_menu     Episode Two menu, `map d1_trainstation_02`, Return to main menu: back over
#                   hl2/background01 (bevy-menu-return.json)
#   chapter_menu    d2_coast_01 returns to hl2/background04, d3_c17_10a to hl2/background06
#                   (bevy-menu-chapter-return.json)
#   chapter_quit    the saved background goes from hl2/background01 to hl2/background04 when
#                   d2_coast_01 is quit mid-game
#   options         every test-inputs/bevy-options-capture*.json captured to artifacts/<name>.png
#   captions        ep1_background02 with closed captions on and off, captured and reported
#   placement       a dragged Options dialog is saved and reopens there in a second launch
#   bg_compare      each game's chapter 1 background at 1024x768, FOV 75, to
#                   artifacts/bg_compare/ for comparison with the retail pre-blurred images
#
# The checks of the window itself (runtime_check, runtime_capture, runtime_resolution_change,
# runtime_window_place) stay separate: they need a window and take focus.
set -uo pipefail
cd "$(dirname "$0")/../.."
OUT="artifacts/replays"
mkdir -p "$OUT" artifacts/bg_compare
CONFIG="bin/hl2-config.json"
BACKUP="$OUT/hl2-config.backup.json"

bash scripts/build-bevy.sh >"$OUT/build.log" 2>&1 || { echo "build failed: $OUT/build.log"; exit 1; }
HAD_CONFIG=0
[ -f "$CONFIG" ] && cp "$CONFIG" "$BACKUP" && HAD_CONFIG=1
restore() {
  if [ "$HAD_CONFIG" -eq 1 ]; then cp "$BACKUP" "$CONFIG"; else rm -f "$CONFIG"; fi
}
trap restore EXIT

# game LOG SECONDS ARGS...: one bounded background run; the watchdog kills it after SECONDS.
game() {
  local log="$1" seconds="$2"
  shift 2
  RUST_LOG="${RUST_LOG:-info}" ./bin/hl2-bevy --no-setup --background "$@" >"$log" 2>&1 &
  local pid=$!
  ( sleep "$seconds"; kill -9 "$pid" ) 2>/dev/null &
  local watchdog=$!
  wait "$pid"
  local status=$?
  # The sleep would otherwise outlive its subshell and hold the output open.
  {
    pkill -P "$watchdog" || true
    kill "$watchdog" || true
    wait "$watchdog" || true
  } 2>/dev/null
  return "$status"
}

# The <game>/<map> the last run ended on (its run report).
current_map() {
  python3 scripts/checks/show_report.py \
    | sed -n 's/^campaign.loaded_game: //p; s/^campaign.loaded_map: //p' | paste -sd/ -
}
# Loaded <game>/<map> lines of the last run, in order.
loaded() { python3 scripts/checks/show_report.py | sed -n 's/^console: Loaded //p' | tr '\n' ' '; }
saved_background() { python3 scripts/checks/show_menu_background.py; }
expect() { # expect DESCRIPTION ACTUAL EXPECTED
  echo "$1: $2"
  [ "$2" = "$3" ] || { echo "expected: $3"; return 1; }
}

check_submenus() {
  game "$OUT/submenus.game.log" 240 --map background01 --main-menu --frames 2400 \
    --movement-script test-inputs/bevy-menu-submenus.json || return
  expect "loaded" "$(loaded)" \
    "hl2/background01 episodic/ep1_background01 hl2/background01 lostcoast/background01 "
}

check_return_menu() {
  game "$OUT/return_menu.game.log" 240 --map background01 --main-menu --frames 3200 \
    --movement-script test-inputs/bevy-menu-return.json || return
  expect "loaded" "$(loaded)" \
    "hl2/background01 ep2/ep2_background01 hl2/d1_trainstation_02 hl2/background01 "
}

check_chapter_menu() {
  game "$OUT/chapter_menu.game.log" 300 --map background01 --main-menu --frames 4400 \
    --movement-script test-inputs/bevy-menu-chapter-return.json || return
  expect "loaded" "$(loaded)" \
    "hl2/background01 hl2/d2_coast_01 hl2/background04 hl2/d3_c17_10a hl2/background06 "
}

check_chapter_quit() {
  BG_GAME=hl2 BG_MAP=background01 python3 scripts/checks/set_menu_background.py
  game "$OUT/chapter_quit.game.log" 180 --map d2_coast_01 --frames 600 || return
  expect "saved menu background" "$(saved_background)" "hl2/background04"
}

check_options() {
  local failed=0 script name
  for script in test-inputs/bevy-options-capture*.json; do
    name="$(basename "$script" .json)"
    rm -f "artifacts/$name.png"
    restore
    if game "$OUT/$name.game.log" 180 --map background01 --main-menu --width 1024 --height 768 \
      --frames 400 --movement-script "$script" --capture "artifacts/$name.png" \
      && [ -s "artifacts/$name.png" ]; then
      echo "captured artifacts/$name.png"
    else
      echo "FAILED $name (see $OUT/$name.game.log)"
      failed=1
    fi
  done
  return "$failed"
}

check_captions() {
  local failed=0 pass png
  for pass in closed_captions:captions_capture none:captions_off; do
    png="artifacts/${pass##*:}.png"
    rm -f "$png"
    restore
    CAPTIONS="${pass%%:*}" python3 scripts/checks/set_captions_config.py
    game "$OUT/${pass##*:}.game.log" 180 --frames 420 --width 1024 --height 768 --capture "$png" \
      || failed=1
    echo "== captions ${pass%%:*}"
    python3 scripts/checks/show_report_captions.py
    [ -s "$png" ] || { echo "no capture $png"; failed=1; }
  done
  return "$failed"
}

check_placement() {
  local open="$OUT/placement_open.json"
  game "$OUT/placement_drag.game.log" 180 --map background01 --main-menu --width 1024 \
    --height 768 --frames 400 --movement-script test-inputs/bevy-options-capture-drag.json \
    --capture artifacts/placement_drag.png || return
  grep -A6 '"dialogs"' "$CONFIG" || { echo "no dialog placement saved"; return 1; }
  # Run 2 only opens Options: the first entries of the plain Options replay.
  { head -7 test-inputs/bevy-options-capture.json | sed '$ s/,$//'; echo "]"; } >"$open"
  game "$OUT/placement_reopen.game.log" 180 --map background01 --main-menu --width 1024 \
    --height 768 --frames 400 --movement-script "$open" --capture artifacts/placement_persist.png \
    || return
  echo "reopened: artifacts/placement_persist.png (the dialog must sit where run 1 left it)"
}

check_bg_compare() {
  local failed=0 pair png
  for pair in hl2:background01 episodic:ep1_background01 ep2:ep2_background01 lostcoast:background01; do
    png="artifacts/bg_compare/${pair%%:*}_${pair##*:}.png"
    rm -f "$png"
    restore
    BG_GAME="${pair%%:*}" BG_MAP="${pair##*:}" BG_FOV=75 python3 scripts/checks/set_bg_config.py
    game "$OUT/bg_compare_${pair%%:*}.game.log" 180 --frames 300 --width 1024 --height 768 \
      --capture "$png" || failed=1
    expect "$pair" "$(current_map)" "${pair%%:*}/${pair##*:}" || failed=1
    [ -s "$png" ] || { echo "no capture $png"; failed=1; }
  done
  return "$failed"
}

CHECKS=(submenus return_menu chapter_menu chapter_quit options captions placement bg_compare)
[ -n "${ONLY:-}" ] && read -r -a CHECKS <<<"$ONLY"
FAILED=0
TOTAL_START=$(date +%s)
for check in "${CHECKS[@]}"; do
  restore
  start=$(date +%s)
  "check_$check" >"$OUT/$check.log" 2>&1
  status=$?
  seconds=$(( $(date +%s) - start ))
  if [ "$status" -eq 0 ]; then
    printf '  ok    %-14s %4d s\n' "$check" "$seconds"
  else
    printf '  FAIL  %-14s %4d s  (see %s/%s.log)\n' "$check" "$seconds" "$OUT" "$check"
    FAILED=$((FAILED + 1))
  fi
done
echo "=== ${#CHECKS[@]} checks, $FAILED failed, $(( $(date +%s) - TOTAL_START )) s ==="
exit "$FAILED"
