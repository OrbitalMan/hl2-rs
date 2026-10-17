#!/usr/bin/env bash
# Tests, strict clippy (CI flags) and fmt for the workspace.
set -euo pipefail
cd "$(dirname "$0")/../.."

echo "=== Running hl2-ui tests ==="
cargo test -p hl2-ui

echo "=== Running hl2-bevy tests ==="
cargo test -p hl2-bevy

echo "=== Running cargo clippy ==="
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "=== Running cargo fmt check ==="
cargo fmt --check

echo "=== All validation checks passed ==="
