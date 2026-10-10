#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <version> [repo-slug] [arm64-sha256] [intel-sha256] [out-file]" >&2
  echo "Example: $0 0.8.0 mgabs/look [arm64-sha] [intel-sha] Casks/look.rb" >&2
  exit 1
fi

VERSION="${1#v}"
REPO_SLUG="${2:-mgabs/look}"
ARM64_SHA="${3:-}"
INTEL_SHA="${4:-}"
OUT_FILE="${5:-Casks/look.rb}"

# If checksums not provided, try reading from local dist/ manifests
if [[ -z "$ARM64_SHA" && -f "dist/Look-${VERSION}-manifest.txt" ]]; then
  ARM64_SHA="$(grep '^sha256=' "dist/Look-${VERSION}-manifest.txt" | cut -d= -f2)"
fi
if [[ -z "$INTEL_SHA" && -f "dist/Look-${VERSION}-manifest-x86_64.txt" ]]; then
  INTEL_SHA="$(grep '^sha256=' "dist/Look-${VERSION}-manifest-x86_64.txt" | cut -d= -f2)"
fi

mkdir -p "$(dirname "$OUT_FILE")"

cat > "$OUT_FILE" <<EOF
cask "look" do
  version "${VERSION}"

  on_arm do
    sha256 "${ARM64_SHA}"
    url "https://github.com/${REPO_SLUG}/releases/download/v#{version}/Look-#{version}-macOS.zip"
  end
  on_intel do
    sha256 "${INTEL_SHA}"
    url "https://github.com/${REPO_SLUG}/releases/download/v#{version}/Look-#{version}-macOS-x86_64.zip"
  end

  name "Look"
  desc "Keyboard-first local launcher"
  homepage "https://github.com/${REPO_SLUG}"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :sequoia

  app "Look.app"
  binary "#{appdir}/Look.app/Contents/MacOS/Look", target: "lookapp"

  uninstall quit: "noah-code.Look"

  zap trash: [
    "~/.look",
    "~/Library/Application Support/Look",
    "~/Library/Caches/noah-code.Look",
    "~/Library/HTTPStorages/noah-code.Look",
    "~/Library/HTTPStorages/noah-code.Look.binarycookies",
    "~/Library/Preferences/noah-code.Look.plist",
  ]
end
EOF

echo "Generated Homebrew cask: $OUT_FILE"
