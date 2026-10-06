#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

DEBUG_BUILD=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug|-d|-DebugBuild)
            DEBUG_BUILD=1
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            echo "Usage: $0 [--debug]" >&2
            exit 1
            ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found. Install Rust using rustup first." >&2
    exit 1
fi

SHADER_SOURCE="$PROJECT_ROOT/crates/hl2-bevy/assets"
if [[ ! -d "$SHADER_SOURCE" ]]; then
    echo "Bevy shader assets are missing from crates/hl2-bevy/assets." >&2
    exit 1
fi

PROFILE_NAME="release"
CARGO_ARGS=("build" "--locked" "-p" "hl2-bevy" "--bin" "hl2-bevy")
if [[ "$DEBUG_BUILD" -eq 1 ]]; then
    PROFILE_NAME="debug"
else
    CARGO_ARGS+=("--release")
fi

echo "Building hl2-bevy ($PROFILE_NAME)..."
cargo "${CARGO_ARGS[@]}"

TARGET_ROOT="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
if [[ -n "${CARGO_BUILD_TARGET:-}" ]]; then
    TARGET_ROOT="$TARGET_ROOT/$CARGO_BUILD_TARGET"
fi

SOURCE_EXECUTABLE="$TARGET_ROOT/$PROFILE_NAME/hl2-bevy"
if [[ ! -f "$SOURCE_EXECUTABLE" ]]; then
    echo "Built executable not found: $SOURCE_EXECUTABLE" >&2
    exit 1
fi

PACKAGE_ROOT="$PROJECT_ROOT/bin"
SHADER_DEST="$PACKAGE_ROOT/bevy-assets"

mkdir -p "$PACKAGE_ROOT"

# Replace shader destination directory so removed shaders cannot linger
rm -rf "$SHADER_DEST"
cp -R "$SHADER_SOURCE" "$SHADER_DEST"
find "$SHADER_DEST" -name ".DS_Store" -delete 2>/dev/null || true

PACKAGE_EXECUTABLE="$PACKAGE_ROOT/hl2-bevy"
cp -f "$SOURCE_EXECUTABLE" "$PACKAGE_EXECUTABLE"
chmod +x "$PACKAGE_EXECUTABLE"

compute_sha256() {
    local file="$1"
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file" | awk '{print toupper($1)}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" | awk '{print toupper($1)}'
    else
        openssl dgst -sha256 "$file" | awk '{print toupper($NF)}'
    fi
}

EXE_SHA256=$(compute_sha256 "$PACKAGE_EXECUTABLE")
BUILT_UTC=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Build shader asset array in JSON
ASSETS_JSON="["
FIRST=1
while IFS= read -r SHADER_FILE; do
    if [[ -f "$SHADER_FILE" ]]; then
        REL_PATH="bin/bevy-assets/${SHADER_FILE#$SHADER_DEST/}"
        FILE_SHA=$(compute_sha256 "$SHADER_FILE")
        if [[ $FIRST -eq 0 ]]; then
            ASSETS_JSON+=","
        fi
        FIRST=0
        ASSETS_JSON+=$'\n    {\n      "path": "'"$REL_PATH"'",\n      "sha256": "'"$FILE_SHA"'"\n    }'
    fi
done < <(find "$SHADER_DEST" -type f ! -name ".*" | sort)
ASSETS_JSON+=$'\n  ]'

METADATA_FILE="$PACKAGE_ROOT/build-bevy-info.json"
TMP_METADATA="$PACKAGE_ROOT/.build-bevy-info-$$.tmp"

cat <<EOF > "$TMP_METADATA"
{
  "built_utc": "$BUILT_UTC",
  "profile": "$PROFILE_NAME",
  "executable": "bin/hl2-bevy",
  "sha256": "$EXE_SHA256",
  "assets": $ASSETS_JSON
}
EOF

mv -f "$TMP_METADATA" "$METADATA_FILE"

echo "Built bin/hl2-bevy with bin/bevy-assets. Run ./launch-bevy.sh."
