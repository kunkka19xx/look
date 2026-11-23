#!/usr/bin/env bash
#
# Formats the macOS app's Swift with swift-format (`.swift-format`): the files
# named, or every tracked one under apps/macos when none are. swift-format
# ships with Xcode, nothing to install.
#
# scripts/manual-tests stays out: swift-format folds the comment under a
# `#!/usr/bin/env swift` line into the shebang, which breaks the script.

set -euo pipefail

readonly CONFIG=".swift-format"
readonly SWIFT_SOURCES="apps/macos/*.swift"

cd "$(dirname "$0")/.."

if ! xcrun --find swift-format >/dev/null 2>&1; then
  echo "format-swift: swift-format not found (it ships with Xcode); skipping" >&2
  exit 0
fi

files=("$@")
if [ "${#files[@]}" -eq 0 ]; then
  while IFS= read -r file; do files+=("$file"); done < <(git ls-files -- "$SWIFT_SOURCES")
fi
[ "${#files[@]}" -eq 0 ] && exit 0

xcrun swift-format format --in-place --parallel --configuration "$CONFIG" "${files[@]}"
