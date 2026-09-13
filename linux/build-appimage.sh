#!/bin/sh
# Builds rustybolt_<version>_<arch>.AppImage from a compiled binary.
# Usage: linux/build-appimage.sh <binary> <version> <x86_64|aarch64> <output.AppImage>
# Needs appimagetool on PATH; the release workflow downloads it.
#
# The AppImage bundles no libraries. GTK and WebKitGTK come from the host:
# WebKitGTK spawns its helper processes from a path compiled into the
# library, so a bundled copy only runs on a host with the build machine's
# layout, and a partial bundle breaks on the first soname mismatch. Every
# current desktop distribution ships webkit2gtk-4.1. The tray needs
# libayatana-appindicator3 on the host; without it the launcher runs with
# no tray.
set -eu

BINARY="$1"
VERSION="$2"
ARCH="$3"
OUT="$4"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APPDIR="$(mktemp -d)/AppDir"

mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/scalable/apps"
cp "$BINARY" "$APPDIR/usr/bin/rustybolt"
cp "$ROOT_DIR/linux/rustybolt.desktop" "$APPDIR/usr/share/applications/rustybolt.desktop"
cp "$ROOT_DIR/linux/rustybolt.desktop" "$APPDIR/rustybolt.desktop"
cp "$ROOT_DIR/icon/rustybolt.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/rustybolt.svg"
cp "$ROOT_DIR/icon/rustybolt.svg" "$APPDIR/rustybolt.svg"
ln -s rustybolt.svg "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/rustybolt" "$@"
RUN
chmod +x "$APPDIR/AppRun"

# The runner has no FUSE; run the appimagetool AppImage from an extracted copy.
export ARCH VERSION
export APPIMAGE_EXTRACT_AND_RUN=1
appimagetool --no-appstream "$APPDIR" "$OUT"

echo "Built $OUT"
