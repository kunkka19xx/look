#!/usr/bin/env bash
# Strip bundled libwayland-* from a Tauri AppImage and repack it.
#
# Tauri's linuxdeploy bundling pulls the build host's libwayland into the
# AppImage. At runtime the host's Mesa EGL driver is loaded into the bundled
# WebKit; when it binds the older bundled libwayland instead of the host
# copy, creating the default EGL display fails and WebKitWebProcess aborts
# with "Could not create default EGL display: EGL_BAD_PARAMETER" (issue
# #233, AppImage on Fedora KDE Wayland). libwayland must always come from
# the host so it matches the host Mesa.
#
# Usage: strip-appimage-wayland-libs.sh <path-to-AppImage>

set -euo pipefail

APPIMAGE="$(realpath "$1")"
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"

# The AppImage runtime and appimagetool need FUSE unless told to
# self-extract; CI runners and containers don't have FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
export ARCH=x86_64

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"

"$APPIMAGE" --appimage-extract >/dev/null

mapfile -t bundled < <(find squashfs-root -name 'libwayland-*.so*')
if [ "${#bundled[@]}" -eq 0 ]; then
    echo "no bundled libwayland in $(basename "$APPIMAGE"), nothing to strip"
    exit 0
fi
rm -v "${bundled[@]}"

curl -fsSL -o appimagetool "$APPIMAGETOOL_URL"
chmod +x appimagetool
./appimagetool squashfs-root repacked.AppImage
mv repacked.AppImage "$APPIMAGE"
