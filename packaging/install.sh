#!/usr/bin/env bash
# Installs Spotty for the current user (no root needed). `./install.sh --uninstall` removes it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/256x256/apps"

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$bin/spotty" "$apps/spotty.desktop" "$icons/spotty.png"
  echo "Removed. Your library (~/.local/share/spotty) and tools (~/.cache/spotty) were kept."
  exit 0
fi

mkdir -p "$bin" "$apps" "$icons"
install -m 755 "$here/spotty" "$bin/spotty"
install -m 644 "$here/icon.png" "$icons/spotty.png"
sed "s|^Exec=.*|Exec=$bin/spotty|" "$here/spotty.desktop" > "$apps/spotty.desktop"
echo "Installed. Start it from your app menu, or run: $bin/spotty"
