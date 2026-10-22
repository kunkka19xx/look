#!/usr/bin/env bash
# Pack a built lookapp binary into an AppImage with linuxdeploy.
#
# Usage: make-appimage.sh <lookapp-binary> <version> <output.AppImage>
#
# linuxdeploy bundles the libraries the binary links (xkbcommon, xcb, alsa).
# What wgpu and the Wayland client open at runtime (the Vulkan loader, GL,
# libwayland) must come from the host so it matches the host's Mesa, which
# is why libwayland is stripped afterwards (issue #233).

set -euo pipefail

BINARY="$(realpath "$1")"
VERSION="$2"
OUTPUT="$(realpath -m "$3")"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LINOWS="$REPO_ROOT/apps/linows"
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"

# linuxdeploy is itself an AppImage; CI runners and containers have no FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
export ARCH=x86_64
export VERSION

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"

curl -fsSL -o linuxdeploy "$LINUXDEPLOY_URL"
chmod +x linuxdeploy

./linuxdeploy --appdir AppDir \
    --executable "$BINARY" \
    --desktop-file "$REPO_ROOT/packaging/linux/lookapp.desktop" \
    --icon-file "$LINOWS/icons/128x128.png" \
    --icon-filename lookapp
# The 256px icon beside it, for HiDPI desktops.
install -Dm644 "$LINOWS/icons/128x128@2x.png" \
    AppDir/usr/share/icons/hicolor/256x256/apps/lookapp.png

OUTPUT="$OUTPUT" ./linuxdeploy --appdir AppDir --output appimage
"$REPO_ROOT/scripts/linux/strip-appimage-wayland-libs.sh" "$OUTPUT"
echo "AppImage written to $OUTPUT"
