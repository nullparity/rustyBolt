#!/bin/sh
# Builds one snapshot for this machine, to prove the release config works.
#
# A release cross compiles with `cargo zigbuild`, which needs Zig. This check
# needs no Zig: it copies the config, swaps the build command for `build`, and
# builds the host target only.
set -eu

cd "$(dirname "$0")/.."

CONFIG="$(mktemp -t rustybolt-goreleaser).yaml"
trap 'rm -f "$CONFIG"' EXIT

sed -e 's/^    command: zigbuild$/    command: build/' \
    -e '/cargo install --locked cargo-zigbuild/d' \
    .goreleaser.yaml > "$CONFIG"

goreleaser build --snapshot --clean --single-target --config "$CONFIG"
echo "The release config builds on this machine."
