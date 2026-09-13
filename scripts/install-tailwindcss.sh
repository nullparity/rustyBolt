#!/usr/bin/env bash
# Installs the pinned Tailwind standalone CLI and checks its SHA-256.
# Usage: scripts/install-tailwindcss.sh [target-dir]   (default: ~/.local/bin)
set -euo pipefail

VERSION=v4.3.3
dir="${1:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  asset=tailwindcss-macos-arm64;  sha=cdf646702987a743464dff4d9c60fd4480d1c1e73dd819a9a67f1078815dce9d ;;
  Darwin-x86_64) asset=tailwindcss-macos-x64;    sha=7922e0953f2110c05976e3bf58f14e643d90427575e766b7d433f5f80cbee7e1 ;;
  Linux-x86_64)  asset=tailwindcss-linux-x64;    sha=dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a ;;
  Linux-aarch64) asset=tailwindcss-linux-arm64;  sha=55fd0b241214eff3de1e8ee4f22796662f2d2e7a49bcfca7477cfd0bac398195 ;;
  MINGW*|MSYS*|CYGWIN*|Windows*) asset=tailwindcss-windows-x64.exe; sha=e0e260ce048014e9268f6237ff18f8ccf02cef521cbd0ae04e82c2cdf7aa3955 ;;
  *) echo "no tailwindcss build for $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

name=tailwindcss
case "$asset" in *.exe) name=tailwindcss.exe ;; esac
mkdir -p "$dir"
curl -fsSL -o "$dir/$name" "https://github.com/tailwindlabs/tailwindcss/releases/download/$VERSION/$asset"
echo "$sha  $dir/$name" | sha256sum -c - >/dev/null 2>&1 || echo "$sha  $dir/$name" | shasum -a 256 -c -
chmod +x "$dir/$name"
"$dir/$name" --help | head -1
