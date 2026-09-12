#!/bin/sh
# Installs rustybolt and desktop integration files for current user.
set -eu

BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"

if [ -f "$SCRIPT_DIR/rustybolt" ]; then
    install -m 755 "$SCRIPT_DIR/rustybolt" "$BIN_DIR/rustybolt"
    echo "Installed binary to $BIN_DIR/rustybolt"
fi

if [ -f "$SCRIPT_DIR/rustybolt.svg" ]; then
    install -m 644 "$SCRIPT_DIR/rustybolt.svg" "$ICON_DIR/rustybolt.svg"
    echo "Installed icon to $ICON_DIR/rustybolt.svg"
fi

if [ -f "$SCRIPT_DIR/rustybolt.desktop" ]; then
    install -m 644 "$SCRIPT_DIR/rustybolt.desktop" "$APP_DIR/rustybolt.desktop"
    echo "Installed desktop entry to $APP_DIR/rustybolt.desktop"
fi

if command -v xdg-mime >/dev/null 2>&1; then
    # The browser hands the `jagex:` login redirect back to the launcher.
    xdg-mime default rustybolt.desktop x-scheme-handler/jagex 2>/dev/null || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" 2>/dev/null || true
fi

echo "Installation complete."
