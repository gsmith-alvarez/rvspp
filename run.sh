#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ -f "$SCRIPT_DIR/target/release/rvs" ]; then
  exec "$SCRIPT_DIR/target/release/rvs" "$@"
elif command -v cargo >/dev/null 2>&1; then
  exec cargo run --release --manifest-path "$SCRIPT_DIR/Cargo.toml" --bin rvs -- "$@"
elif command -v nix >/dev/null 2>&1; then
  exec nix run "$SCRIPT_DIR" -- "$@"
else
  echo "Error: Neither cargo nor nix found to run rvs." >&2
  exit 1
fi
