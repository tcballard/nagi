#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ${1:-} == --uninstall ]]; then
  make uninstall PREFIX="$HOME/.local"
  exit
fi
cargo build --release --locked
make install PREFIX="$HOME/.local"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
printf 'Installed Nagi. Launch with %s/.local/bin/nagi\n' "$HOME"
