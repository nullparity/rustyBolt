#!/bin/sh
# Builds a .dmg that holds rustyBolt.app and a link to /Applications.
set -eu

APP="${1:-target/release/rustyBolt.app}"
VERSION="${2:-0.1.0}"
DMG="${3:-$(dirname "$APP")/rustyBolt.dmg}"

if [ ! -d "$APP" ]; then
    echo "rustyBolt.app not found at $APP" >&2
    exit 1
fi

STAGE="$DMG.stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"
trap 'rm -rf "$STAGE"' EXIT

cp -R "$APP" "$STAGE/rustyBolt.app"
ln -s /Applications "$STAGE/Applications"

rm -f "$DMG"
hdiutil create -quiet -volname "rustyBolt $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"

echo "Built $DMG"
