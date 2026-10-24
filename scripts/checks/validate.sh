#!/usr/bin/env bash
# Architecture guardrails, tests, strict clippy (CI flags) and fmt for the workspace.
set -euo pipefail
cd "$(dirname "$0")/../.."

echo "=== Checking architecture guardrails ==="
python3 scripts/checks/architecture.py

echo "=== Running workspace tests ==="
cargo test --workspace --locked

echo "=== Running cargo clippy ==="
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "=== Running cargo fmt check ==="
cargo fmt --check

echo "=== All validation checks passed ==="
