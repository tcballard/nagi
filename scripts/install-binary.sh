#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
prefix="$HOME/.local"
if [[ ${1:-} == --uninstall ]]; then
  rm -f "$prefix/bin/nagi" "$prefix/share/applications/io.github.tcballard.Nagi.desktop" "$prefix/share/icons/hicolor/scalable/apps/io.github.tcballard.Nagi.svg" "$prefix/share/licenses/nagi/LICENSE"
  ./scripts/install-icons.sh "$prefix/share/icons/hicolor" --uninstall
  printf 'Nagi removed. Your browsing data has been kept.\n'
  exit
fi
install -Dm755 nagi "$prefix/bin/nagi"
install -Dm644 packaging/io.github.tcballard.Nagi.desktop "$prefix/share/applications/io.github.tcballard.Nagi.desktop"
./scripts/install-icons.sh "$prefix/share/icons/hicolor"
install -Dm644 LICENSE "$prefix/share/licenses/nagi/LICENSE"
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
printf 'Installed Nagi. Launch with %s/bin/nagi\n' "$prefix"
