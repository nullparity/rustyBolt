#!/bin/sh
# Makes rustyBolt.app from the release binary.
#
# The bundle holds the Rust binary itself. No shell script starts it, so the Dock
# owns the real process and macOS gives it the window and the icon.
set -eu

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
DEST="${1:-$ROOT/target}"
APP="$DEST/rustyBolt.app"

if [ ! -f "$ROOT/icon/rustybolt.icns" ]; then
    echo "The icon is absent. Run icon/build.sh first." >&2
    exit 1
fi

cargo build --release -p bolt-macos

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$ROOT/target/release/rustybolt-macos" "$APP/Contents/MacOS/rustybolt-macos"
cp "$ROOT/icon/rustybolt.icns" "$APP/Contents/Resources/rustybolt.icns"
cp "$ROOT/macos/Info.plist" "$APP/Contents/Info.plist"

plutil -lint "$APP/Contents/Info.plist" >/dev/null

# An ad hoc signature stops the "damaged" message of a local build.
codesign --force --deep --sign - "$APP" 2>/dev/null || \
    echo "The ad hoc signature failed. The bundle still runs after you allow it." >&2

touch "$APP"
echo "Made $APP"
