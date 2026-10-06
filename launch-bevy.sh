#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [[ ! -f "bin/hl2-bevy" ]]; then
    echo "Build with scripts/build-bevy.sh first." >&2
    exit 1
fi

exec "./bin/hl2-bevy" "$@"
