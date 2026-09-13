#!/bin/sh
# Builds rustybolt_<version>_<arch>.deb from a compiled binary.
# Usage: linux/build-deb.sh <binary> <version> <amd64|arm64> <output.deb>
set -eu

BINARY="$1"
VERSION="$2"
ARCH="$3"
OUT="$4"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

install -Dm755 "$BINARY" "$STAGE/usr/bin/rustybolt"
install -Dm644 "$ROOT_DIR/linux/rustybolt.desktop" "$STAGE/usr/share/applications/rustybolt.desktop"
install -Dm644 "$ROOT_DIR/icon/rustybolt.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/rustybolt.svg"

mkdir -p "$STAGE/DEBIAN"
cat > "$STAGE/DEBIAN/control" <<CONTROL
Package: rustybolt
Version: $VERSION
Section: games
Priority: optional
Architecture: $ARCH
Maintainer: nullparity <noreply@github.com>
Homepage: https://github.com/nullparity/rustyBolt
Depends: libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1, libdbus-1-3
Recommends: gnome-keyring | kwalletd6 | kwalletd5 | keepassxc
Description: Launcher for RuneLite and HDOS
 A free, portable launcher that logs you in to your Jagex account
 and starts the RuneLite or HDOS client you already installed.
CONTROL

# The browser hands the `jagex:` login redirect back to the launcher.
cat > "$STAGE/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
exit 0
POSTINST
chmod 755 "$STAGE/DEBIAN/postinst"

dpkg-deb --build --root-owner-group "$STAGE" "$OUT"
echo "Built $OUT"
