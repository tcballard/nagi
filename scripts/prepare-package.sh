#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/package-source.sh
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
digest=$(sha256sum "dist/nagi-$version.tar.gz" | cut -d' ' -f1)
sed "s/@SHA256@/$digest/" packaging/PKGBUILD.in > dist/PKGBUILD
printf 'On Arch/Omarchy: cd dist && makepkg -si\n'
