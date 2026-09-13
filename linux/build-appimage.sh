#!/bin/sh
# Builds rustybolt_<version>_<arch>.AppImage from a compiled binary.
# Usage: linux/build-appimage.sh <binary> <version> <x86_64|aarch64> <output.AppImage>
# Needs linuxdeploy and linuxdeploy-plugin-gtk on PATH; the release workflow downloads them.
set -eu

BINARY="$1"
VERSION="$2"
ARCH="$3"
OUT="$4"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APPDIR="$(mktemp -d)/AppDir"
mkdir -p "$APPDIR"

export ARCH VERSION
export DEPLOY_GTK_VERSION=3
export OUTPUT="$OUT"
# The runner has no FUSE; run the linuxdeploy AppImage from an extracted copy.
export APPIMAGE_EXTRACT_AND_RUN=1

linuxdeploy --appdir "$APPDIR" \
    --executable "$BINARY" \
    --desktop-file "$ROOT_DIR/linux/rustybolt.desktop" \
    --icon-file "$ROOT_DIR/icon/rustybolt.svg" \
    --plugin gtk \
    --output appimage

echo "Built $OUT"
