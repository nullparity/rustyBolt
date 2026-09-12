#!/bin/sh
# Makes rustybolt.icns from rustybolt.svg.
#
# Needs `rsvg-convert` from the librsvg package and `iconutil` from macOS.
set -eu

cd "$(dirname "$0")"

if ! command -v rsvg-convert >/dev/null 2>&1; then
    echo "rsvg-convert is absent. Run: brew install librsvg" >&2
    exit 1
fi

SET="rustybolt.iconset"
rm -rf "$SET"
mkdir "$SET"

render() {
    rsvg-convert -w "$1" -h "$1" rustybolt.svg -o "$SET/$2"
}

render 16 icon_16x16.png
render 32 icon_16x16@2x.png
render 32 icon_32x32.png
render 64 icon_32x32@2x.png
render 128 icon_128x128.png
render 256 icon_128x128@2x.png
render 256 icon_256x256.png
render 512 icon_256x256@2x.png
render 512 icon_512x512.png
render 1024 icon_512x512@2x.png

iconutil --convert icns --output rustybolt.icns "$SET"
rm -rf "$SET"
echo "Made $(pwd)/rustybolt.icns"
