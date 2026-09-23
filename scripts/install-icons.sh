#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
icons=${1:?Pass the hicolor installation directory}
paths=(scalable/apps/nagi.svg symbolic/apps/nagi-symbolic.svg)
for size in 16 22 24 32 48 64 128 256 512; do
  paths+=("${size}x${size}/apps/nagi.png")
done
for relative in "${paths[@]}"; do
  if [[ ${2:-} == --uninstall ]]; then
    rm -f "$icons/$relative"
  else
    install -Dm644 "$root/assets/icons/hicolor/$relative" "$icons/$relative"
  fi
done
# Remove the icon installed by the original v0.0.1 package/user installer.
rm -f "$icons/scalable/apps/io.github.tcballard.Nagi.svg"
if [[ -f "$icons/icon-theme.cache" ]] && command -v gtk-update-icon-cache >/dev/null; then
  gtk-update-icon-cache -f "$icons" || true
fi
