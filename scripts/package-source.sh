#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
mkdir -p dist
git archive --format=tar.gz --prefix="nagi-$version/" HEAD > "dist/nagi-$version.tar.gz"
sha256sum "dist/nagi-$version.tar.gz"
