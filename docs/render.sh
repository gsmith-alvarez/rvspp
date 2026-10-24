#!/usr/bin/env bash
# Regenerate the README screenshots and the demo GIF.
# Requires VHS (https://github.com/charmbracelet/vhs); the Nix package bundles
# ttyd, ffmpeg and Chromium, so `nix run nixpkgs#vhs` is enough.
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --release --bin rvs

if command -v vhs >/dev/null 2>&1; then
  vhs_run() { vhs "$1"; }
else
  vhs_run() { nix run nixpkgs#vhs -- "$1"; }
fi

for tape in demo screens screens-zoom screens-gnu; do
  vhs_run "docs/$tape.tape"
done

# Optional compression: flat-colour terminal captures shrink ~4x.
if command -v pngquant >/dev/null 2>&1; then
  for png in docs/img/*.png; do
    pngquant --force --quality=70-95 --output "$png" -- "$png"
  done
fi
if command -v gifsicle >/dev/null 2>&1; then
  gifsicle -O3 --lossy=60 --colors 200 docs/img/demo.gif -o docs/img/demo.gif
fi

ls -lh docs/img
