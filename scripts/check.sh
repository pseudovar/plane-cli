#!/usr/bin/env bash
set -euo pipefail

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy --locked --workspace --all-targets -- -D warnings"
cargo clippy --locked --workspace --all-targets -- -D warnings

echo "==> cargo test --locked --workspace"
cargo test --locked --workspace

echo "All checks passed."
