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

# WebKitGTK runs its network and web processes as helper executables and
# hard-codes the build host's path to them. Bundle them, and point WebKit
# at the bundle from AppRun, or the AppImage only starts on Debian-family
# hosts with the same paths.
MULTIARCH="$(gcc -print-multiarch)"
WEBKIT_DIR="/usr/lib/$MULTIARCH/webkit2gtk-4.1"

linuxdeploy --appdir "$APPDIR" \
    --executable "$BINARY" \
    --executable "$WEBKIT_DIR/WebKitNetworkProcess" \
    --executable "$WEBKIT_DIR/WebKitWebProcess" \
    --library "$WEBKIT_DIR/injected-bundle/libwebkit2gtkinjectedbundle.so" \
    --desktop-file "$ROOT_DIR/linux/rustybolt.desktop" \
    --icon-file "$ROOT_DIR/icon/rustybolt.svg" \
    --plugin gtk

# linuxdeploy keeps an AppRun that already exists, so the hook survives the
# packaging pass below.
sed -i '/^exec /i\
export WEBKIT_EXEC_PATH="$this_dir/usr/bin"\
export WEBKIT_INJECTED_BUNDLE_PATH="$this_dir/usr/lib"' "$APPDIR/AppRun"

linuxdeploy --appdir "$APPDIR" --output appimage

echo "Built $OUT"
