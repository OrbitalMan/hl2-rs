#!/usr/bin/env bash
# Runs every background replay and capture check in sequence (no window, nothing takes focus)
# and prints one pass/fail summary with durations. Each check's full output goes to
# artifacts/runtime_all/<check>.log. The windowed checks of the window itself
# (runtime_check, runtime_capture, runtime_resolution_change, runtime_window_place) are not
# included: they take focus.
set -uo pipefail
cd "$(dirname "$0")/../.."
mkdir -p artifacts/runtime_all
CHECKS=(
  runtime_submenus
  runtime_return_menu
  runtime_chapter_menu
  runtime_chapter_quit
  runtime_options_capture
  runtime_captions
  runtime_placement_persist
  runtime_bg_compare
)
FAILED=0
TOTAL_START=$(date +%s)
for check in "${CHECKS[@]}"; do
  start=$(date +%s)
  bash "scripts/checks/$check.sh" >"artifacts/runtime_all/$check.log" 2>&1
  status=$?
  seconds=$(( $(date +%s) - start ))
  if [ "$status" -eq 0 ]; then
    printf '  ok    %-28s %4d s\n' "$check" "$seconds"
  else
    printf '  FAIL  %-28s %4d s  (exit %d, see artifacts/runtime_all/%s.log)\n' \
      "$check" "$seconds" "$status" "$check"
    FAILED=$((FAILED + 1))
  fi
done
echo "=== ${#CHECKS[@]} checks, $FAILED failed, $(( $(date +%s) - TOTAL_START )) s ==="
exit "$FAILED"
