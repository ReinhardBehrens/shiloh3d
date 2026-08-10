#!/usr/bin/env bash
# Install Shiloh3D desktop icons so the OS start bar / app launcher
# shows the same mark as logo_shiloh3d.png.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SRC="$ROOT/packaging/icons/shiloh3d.png"
ICO_SRC="$ROOT/packaging/icons/shiloh3d.ico"
DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_BASE="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"

install -d "$DESKTOP_DIR"
install -m 644 "$ROOT/packaging/linux/shiloh-editor.desktop" "$DESKTOP_DIR/"
install -m 644 "$ROOT/packaging/linux/shiloh-demo.desktop" "$DESKTOP_DIR/"

for size in 16 32 48 64 128 256; do
  dest="$ICON_BASE/${size}x${size}/apps"
  install -d "$dest"
  if [[ -f "$ROOT/packaging/icons/shiloh3d-${size}.png" ]]; then
    install -m 644 "$ROOT/packaging/icons/shiloh3d-${size}.png" "$dest/shiloh3d.png"
  else
    install -m 644 "$ICON_SRC" "$dest/shiloh3d.png"
  fi
done
install -d "$ICON_BASE/512x512/apps"
install -m 644 "$ICON_SRC" "$ICON_BASE/512x512/apps/shiloh3d.png"

# Optional Windows copy hint
echo "Linux icons + .desktop installed under $DESKTOP_DIR"
echo "Windows: use packaging/icons/shiloh3d.ico for shortcuts ($ICO_SRC)"
command -v update-desktop-database >/dev/null && update-desktop-database "$DESKTOP_DIR" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$ICON_BASE" 2>/dev/null || true
