#!/usr/bin/env bash
# Builds a release tarball: dist/spotty-<version>-linux-<arch>.tar.gz (+ .sha256)
set -euo pipefail
cd "$(dirname "$0")/.."
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
name="spotty-$version-linux-$(uname -m)"
out="dist/$name"

cargo build --release
rm -rf "$out" && mkdir -p "$out"
cp target/release/spotty "$out/spotty" && strip "$out/spotty"
cp packaging/spotty.desktop packaging/install.sh README.md "$out/"
cp assets/icon.png "$out/icon.png"
tar -C dist -czf "$out.tar.gz" "$name"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "built $out.tar.gz"
