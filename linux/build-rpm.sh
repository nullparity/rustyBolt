#!/bin/sh
# Builds rustybolt-<version>.<arch>.rpm from a compiled binary.
# Usage: linux/build-rpm.sh <binary> <version> <x86_64|aarch64> <output.rpm>
# Needs rpmbuild; the release workflow installs the rpm package on Ubuntu.
#
# The Requires name shared libraries by soname (libwebkit2gtk-4.1.so.0 and
# so on), not packages, so the rpm installs on Fedora, openSUSE and their
# relatives alike. rpmbuild adds the rest of the sonames it finds in the
# binary.
set -eu

BINARY="$1"
VERSION="$2"
ARCH="$3"
OUT="$4"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TOP="$(mktemp -d)"
trap 'rm -rf "$TOP"' EXIT
mkdir -p "$TOP/SOURCES" "$TOP/SPECS" "$TOP/BUILD" "$TOP/RPMS"
cp "$BINARY" "$TOP/SOURCES/rustybolt"
cp "$ROOT_DIR/linux/rustybolt.desktop" "$ROOT_DIR/icon/rustybolt.svg" "$TOP/SOURCES/"

cat > "$TOP/SPECS/rustybolt.spec" <<SPEC
Name:           rustybolt
Version:        $VERSION
Release:        1
Summary:        Launcher for RuneLite and HDOS
License:        MIT
URL:            https://github.com/nullparity/rustyBolt
Source0:        rustybolt
Source1:        rustybolt.desktop
Source2:        rustybolt.svg
Requires:       libwebkit2gtk-4.1.so.0()(64bit)
Requires:       libgtk-3.so.0()(64bit)
Requires:       libdbus-1.so.3()(64bit)
Recommends:     gnome-keyring
Recommends:     libayatana-appindicator-gtk3
# The binary is prebuilt; do not strip or rebuild the debug info.
%global debug_package %{nil}
%global __strip /bin/true

%description
A free, portable launcher that logs you in to your Jagex account
and starts the RuneLite or HDOS client you already installed.

%install
mkdir -p %{buildroot}%{_bindir} %{buildroot}%{_datadir}/applications %{buildroot}%{_datadir}/icons/hicolor/scalable/apps
install -m755 %{SOURCE0} %{buildroot}%{_bindir}/rustybolt
install -m644 %{SOURCE1} %{buildroot}%{_datadir}/applications/rustybolt.desktop
install -m644 %{SOURCE2} %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/rustybolt.svg

%files
%{_bindir}/rustybolt
%{_datadir}/applications/rustybolt.desktop
%{_datadir}/icons/hicolor/scalable/apps/rustybolt.svg
SPEC

rpmbuild -bb --define "_topdir $TOP" --define "_prefix /usr" --target "$ARCH" "$TOP/SPECS/rustybolt.spec" >/dev/null
cp "$TOP/RPMS/$ARCH/rustybolt-$VERSION-1.$ARCH.rpm" "$OUT"
echo "Built $OUT"
