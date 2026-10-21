#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
cd "$PROJECT_ROOT"

FORCE_REBUILD=0
DEBUG_BUILD=0
NO_BUILD=0
CLEAN_TARGET=0
CLEAN_MODE="stale"
GAME_ARGS=()

for arg in "$@"; do
    case "$arg" in
        --rebuild|--force-build)
            FORCE_REBUILD=1
            ;;
        --debug|-d)
            DEBUG_BUILD=1
            ;;
        --no-build)
            NO_BUILD=1
            ;;
        --clean)
            CLEAN_TARGET=1
            CLEAN_MODE="stale"
            ;;
        --clean=workspace|--clean-workspace)
            CLEAN_TARGET=1
            CLEAN_MODE="workspace"
            ;;
        --clean=dry-run|--clean-dry-run)
            CLEAN_TARGET=1
            CLEAN_MODE="dry-run"
            ;;
        *)
            GAME_ARGS+=("$arg")
            ;;
    esac
done

TARGET_BIN="$PROJECT_ROOT/bin/hl2-bevy"
INFO_JSON="$PROJECT_ROOT/bin/build-bevy-info.json"
BEVY_ASSETS="$PROJECT_ROOT/bin/bevy-assets"
SHADER_SOURCE="$PROJECT_ROOT/crates/hl2-bevy/assets"
CLEANER_SCRIPT="$PROJECT_ROOT/scripts/clean-target.sh"

DESIRED_PROFILE="release"
if [[ "$DEBUG_BUILD" -eq 1 ]]; then
    DESIRED_PROFILE="debug"
fi

# Clean remnants from old/aborted builds in bin/
rm -f "$PROJECT_ROOT/bin/".*.tmp "$PROJECT_ROOT/bin/"*.tmp "$PROJECT_ROOT/bin/"*.old "$PROJECT_ROOT/bin/"*.bak 2>/dev/null || true
find "$PROJECT_ROOT/bin" -name ".DS_Store" -delete 2>/dev/null || true

# Explicit --clean requested
if [[ "$CLEAN_TARGET" -eq 1 ]]; then
    if [[ -x "$CLEANER_SCRIPT" ]]; then
        echo "==> Cleaning target directory remnants (mode: $CLEAN_MODE)..."
        if [[ "$CLEAN_MODE" == "workspace" ]]; then
            "$CLEANER_SCRIPT" --workspace
        elif [[ "$CLEAN_MODE" == "dry-run" ]]; then
            "$CLEANER_SCRIPT" --dry-run
        else
            "$CLEANER_SCRIPT" --stale
        fi
    else
        echo "Warning: Cleaner script not found at $CLEANER_SCRIPT" >&2
    fi
    # If only --clean was provided with no other args, exit cleanly
    if [[ ${#GAME_ARGS[@]} -eq 0 && "$FORCE_REBUILD" -eq 0 && "$DEBUG_BUILD" -eq 0 && "$NO_BUILD" -eq 0 ]]; then
        exit 0
    fi
fi

NEEDS_BUILD=0
BUILD_REASON=""
BRANCH_CHANGED=0

CURRENT_BRANCH="$(git symbolic-ref --short -q HEAD 2>/dev/null || git rev-parse --short HEAD 2>/dev/null || echo "unknown")"
CURRENT_COMMIT="$(git rev-parse HEAD 2>/dev/null || echo "unknown")"

if [[ "$NO_BUILD" -eq 1 ]]; then
    if [[ ! -x "$TARGET_BIN" ]]; then
        echo "Error: --no-build was specified, but executable does not exist: $TARGET_BIN" >&2
        exit 1
    fi
elif [[ "$FORCE_REBUILD" -eq 1 ]]; then
    NEEDS_BUILD=1
    BUILD_REASON="force rebuild requested (--rebuild)"
elif [[ ! -f "$TARGET_BIN" || ! -x "$TARGET_BIN" ]]; then
    NEEDS_BUILD=1
    BUILD_REASON="executable missing or not executable ($TARGET_BIN)"
elif [[ ! -d "$BEVY_ASSETS" ]]; then
    NEEDS_BUILD=1
    BUILD_REASON="shader assets directory missing ($BEVY_ASSETS)"
elif [[ ! -f "$INFO_JSON" ]]; then
    NEEDS_BUILD=1
    BUILD_REASON="build metadata missing ($INFO_JSON)"
else
    # 1. Check profile mismatch (release vs debug)
    RECORDED_PROFILE="$(grep -E '"profile"' "$INFO_JSON" 2>/dev/null | head -n1 | sed -E 's/.*"profile"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/' || true)"
    if [[ -n "$RECORDED_PROFILE" && "$RECORDED_PROFILE" != "$DESIRED_PROFILE" ]]; then
        NEEDS_BUILD=1
        BUILD_REASON="build profile changed (recorded: '$RECORDED_PROFILE', desired: '$DESIRED_PROFILE')"
    fi

    # 2. Check git branch changes
    if [[ "$NEEDS_BUILD" -eq 0 && "$CURRENT_BRANCH" != "unknown" ]]; then
        RECORDED_BRANCH="$(grep -E '"git_branch"' "$INFO_JSON" 2>/dev/null | head -n1 | sed -E 's/.*"git_branch"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/' || true)"
        if [[ -z "$RECORDED_BRANCH" ]]; then
            NEEDS_BUILD=1
            BUILD_REASON="metadata missing git branch information"
        elif [[ "$CURRENT_BRANCH" != "$RECORDED_BRANCH" ]]; then
            NEEDS_BUILD=1
            BRANCH_CHANGED=1
            BUILD_REASON="git branch changed ('$RECORDED_BRANCH' -> '$CURRENT_BRANCH')"
        fi
    fi

    # 3. Check git commit changes
    if [[ "$NEEDS_BUILD" -eq 0 && "$CURRENT_COMMIT" != "unknown" ]]; then
        RECORDED_COMMIT="$(grep -E '"git_commit"' "$INFO_JSON" 2>/dev/null | head -n1 | sed -E 's/.*"git_commit"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/' || true)"
        if [[ -z "$RECORDED_COMMIT" ]]; then
            NEEDS_BUILD=1
            BUILD_REASON="metadata missing git commit information"
        elif [[ "$CURRENT_COMMIT" != "$RECORDED_COMMIT" ]]; then
            NEEDS_BUILD=1
            BUILD_REASON="git commit changed ('${RECORDED_COMMIT:0:8}' -> '${CURRENT_COMMIT:0:8}')"
        fi
    fi

    # 4. Check modified source files (newer than current binary)
    if [[ "$NEEDS_BUILD" -eq 0 ]]; then
        NEWER_SOURCE="$(find "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" "$PROJECT_ROOT/Cargo.lock" -newer "$TARGET_BIN" -type f ! -name ".*" 2>/dev/null | head -n 1 || true)"
        if [[ -n "$NEWER_SOURCE" ]]; then
            NEEDS_BUILD=1
            BUILD_REASON="source file modified (${NEWER_SOURCE#$PROJECT_ROOT/})"
        fi
    fi

    # 5. Check modified shader assets
    if [[ "$NEEDS_BUILD" -eq 0 && -d "$SHADER_SOURCE" ]]; then
        NEWER_SHADER="$(find "$SHADER_SOURCE" -newer "$TARGET_BIN" -type f ! -name ".*" 2>/dev/null | head -n 1 || true)"
        if [[ -n "$NEWER_SHADER" ]]; then
            NEEDS_BUILD=1
            BUILD_REASON="shader asset modified (${NEWER_SHADER#$PROJECT_ROOT/})"
        fi
    fi
fi

# Auto-prune stale remnants from earlier branches before rebuilding
if [[ "$BRANCH_CHANGED" -eq 1 && -x "$CLEANER_SCRIPT" ]]; then
    echo "==> Git branch changed ('$RECORDED_BRANCH' -> '$CURRENT_BRANCH'): auto-pruning stale hashes..."
    "$CLEANER_SCRIPT" --stale || true
fi

if [[ "$NEEDS_BUILD" -eq 1 ]]; then
    echo "==> Build is out of date: $BUILD_REASON"
    echo "==> Running scripts/build-bevy.sh..."
    BUILD_ARGS=()
    if [[ "$DEBUG_BUILD" -eq 1 ]]; then
        BUILD_ARGS+=("--debug")
    fi
    if [[ ${#BUILD_ARGS[@]} -gt 0 ]]; then
        "$PROJECT_ROOT/scripts/build-bevy.sh" "${BUILD_ARGS[@]}"
    else
        "$PROJECT_ROOT/scripts/build-bevy.sh"
    fi
else
    echo "==> Build is up to date (${DESIRED_PROFILE} on branch '$CURRENT_BRANCH', commit '${CURRENT_COMMIT:0:8}')."
fi

if [[ ! -x "$TARGET_BIN" ]]; then
    echo "Error: executable not found or not executable: $TARGET_BIN" >&2
    exit 1
fi

if [[ ${#GAME_ARGS[@]} -gt 0 ]]; then
    exec "$TARGET_BIN" "${GAME_ARGS[@]}"
else
    exec "$TARGET_BIN"
fi
