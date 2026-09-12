#!/bin/sh
# Builds rustyBolt.app from a compiled binary and metadata.
set -eu

BINARY="${1:-target/release/rustybolt}"
VERSION="${2:-0.1.0}"
APP="${3:-$(dirname "$BINARY")/rustyBolt.app}"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [ ! -f "$BINARY" ]; then
    echo "rustybolt binary not found at $BINARY" >&2
    exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/Resources"

cp "$BINARY" "$APP/Contents/MacOS/rustybolt"
chmod +x "$APP/Contents/MacOS/rustybolt"

cp "$ROOT_DIR/icon/rustybolt.icns" "$APP/Contents/Resources/rustybolt.icns"
cp "$ROOT_DIR/macos/install-login-redirect.sh" "$APP/Contents/Resources/install-login-redirect.sh"
chmod +x "$APP/Contents/Resources/install-login-redirect.sh"

sed "s/VERSION_PLACEHOLDER/$VERSION/g" "$ROOT_DIR/macos/Info.plist" > "$APP/Contents/Info.plist"

echo "Built $APP"
