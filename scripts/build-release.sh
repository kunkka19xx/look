#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version>" >&2
  echo "Example: $0 1.0.0" >&2
  exit 1
fi

"$ROOT_DIR/scripts/release-macos-app.sh" "$VERSION"

ZIP_PATH="$ROOT_DIR/dist/Look-${VERSION}-macOS.zip"
if [[ ! -f "$ZIP_PATH" ]]; then
  echo "Expected artifact missing: $ZIP_PATH" >&2
  exit 1
fi

SHA256="$(shasum -a 256 "$ZIP_PATH" | awk '{print $1}')"
MANIFEST_PATH="$ROOT_DIR/dist/Look-${VERSION}-manifest.txt"

cat > "$MANIFEST_PATH" <<EOF
version=${VERSION}
artifact=$(basename "$ZIP_PATH")
sha256=${SHA256}
EOF

echo
echo "Release manifest written: $MANIFEST_PATH"
