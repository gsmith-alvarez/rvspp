#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if command -v wish >/dev/null 2>&1; then
  exec wish "$SCRIPT_DIR/src/RVS046.tcl" "$@"
else
  exec nix-shell -p tk --run "wish \"$SCRIPT_DIR/src/RVS046.tcl\" $*"
fi
