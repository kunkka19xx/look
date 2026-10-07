#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APP_DIR="$ROOT_DIR/apps/macos/LauncherApp"
SCHEME="Look"
CONFIGURATION="Release"

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  VERSION="$(date +%Y.%m.%d)"
fi

# shellcheck source=scripts/release-arch.sh
source "$ROOT_DIR/scripts/release-arch.sh"

BUILD_DIR="$ROOT_DIR/.build/release-macos"
OUT_DIR="$ROOT_DIR/dist"
DERIVED_DATA="$BUILD_DIR/DerivedData"
APP_PATH="$DERIVED_DATA/Build/Products/$CONFIGURATION/Look.app"
ZIP_NAME="Look-${VERSION}-macOS${ARCH_SUFFIX}.zip"
ZIP_PATH="$OUT_DIR/$ZIP_NAME"

echo "[1/4] Cleaning previous release artifacts"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR" "$OUT_DIR"

echo "[2/4] Building macOS app ($CONFIGURATION, $ARCHS)"
# -destination "generic/platform=macOS" is "Any Mac". Without it xcodebuild
# resolves the concrete "My Mac" destination and filters ARCHS down to the host
# arch, which is why every release so far has been arm64 only. ARCHS is pinned
# explicitly because the Release default is ARCHS_STANDARD (arm64 x86_64), and
# we publish one asset per architecture rather than a fat binary.
xcodebuild \
  -project "$APP_DIR/look-app.xcodeproj" \
  -scheme "$SCHEME" \
  -configuration "$CONFIGURATION" \
  -destination "generic/platform=macOS" \
  -derivedDataPath "$DERIVED_DATA" \
  ARCHS="$ARCHS" \
  MARKETING_VERSION="$VERSION" \
  CURRENT_PROJECT_VERSION="$VERSION" \
  build >/dev/null

if [[ ! -d "$APP_PATH" ]]; then
  echo "Build succeeded but app bundle not found at: $APP_PATH" >&2
  exit 1
fi

BUILT_ARCHS="$(lipo -archs "$APP_PATH/Contents/MacOS/Look")"
if [[ "$BUILT_ARCHS" != "$ARCHS" ]]; then
  echo "Expected a $ARCHS binary, got: $BUILT_ARCHS" >&2
  exit 1
fi

echo "[3/4] Packaging app bundle"
rm -f "$ZIP_PATH"
ditto -c -k --sequesterRsrc --keepParent "$APP_PATH" "$ZIP_PATH"

echo "[4/4] Calculating sha256"
SHA256="$(shasum -a 256 "$ZIP_PATH" | awk '{print $1}')"

echo
echo "Release artifact ready:"
echo "  File: $ZIP_PATH"
echo "  SHA256: $SHA256"
echo
echo "Use this URL+SHA256 in your Homebrew cask."
