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

# rustyBolt opens one helper per client through Launch Services, so macOS
# shows each client as its own app and not as a second rustyBolt. The Dock
# takes the name from the folder name of the helper. Each helper executable
# is a hard link to the launcher binary. A dmg stores each link as a copy.
for NAME in RuneLite HDOS; do
    ID=$(echo "$NAME" | tr '[:upper:]' '[:lower:]')
    HELPER="$APP/Contents/Helpers/$NAME.app"
    mkdir -p "$HELPER/Contents/MacOS"
    ln "$APP/Contents/MacOS/rustybolt" "$HELPER/Contents/MacOS/$NAME"
    sed -e "s/VERSION_PLACEHOLDER/$VERSION/g" -e "s/NAME_PLACEHOLDER/$NAME/g" -e "s/ID_PLACEHOLDER/$ID/g" \
        "$ROOT_DIR/macos/Client-Info.plist" > "$HELPER/Contents/Info.plist"
done

echo "Built $APP"
