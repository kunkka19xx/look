#!/usr/bin/env bash
# Point packaging/nix/package-darwin.nix at a released macOS build.
#
# Rewrites `version` and the hash of every architecture the file already
# carries, in one pass, because the two are only ever correct as a pair: a
# bumped version with a stale hash fails the build, and a stale version with a
# fresh hash silently keeps Nix users on the old release. Every edit lands on a
# copy and is verified before it replaces the file, so a failure part way
# through leaves no half-bumped pin behind.
#
#   scripts/nix-bump-darwin.sh 0.7.3
#
# Exits 3 when a release asset exists for an architecture the file does not
# package yet (today: Intel). The bump itself is written in that case, so the
# caller can land it and treat 3 as "add that entry by hand".
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FILE="$ROOT_DIR/packaging/nix/package-darwin.nix"
DOWNLOAD_URL="https://github.com/kunkka19xx/look/releases/download"

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "usage: $(basename "$0") <version>   (for example 0.7.3)" >&2
  exit 2
fi
VERSION="${VERSION#v}"

# system:asset-suffix, mirroring ARCH_SUFFIX in scripts/release-arch.sh.
ARCHES=("aarch64-darwin:" "x86_64-darwin:-x86_64")

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
draft="$WORK/package-darwin.nix"
cp "$FILE" "$draft"

unpackaged=()

for entry in "${ARCHES[@]}"; do
  arch="${entry%%:*}"
  suffix="${entry#*:}"
  url="$DOWNLOAD_URL/v$VERSION/Look-$VERSION-macOS$suffix.zip"

  if ! grep -q "^    $arch = {" "$draft"; then
    if curl -fsLI -o /dev/null "$url"; then
      unpackaged+=("$arch -> $url")
    fi
    continue
  fi

  echo "prefetching $url"
  hash="$(nix store prefetch-file --json "$url" | jq -r .hash)"
  if [[ -z "$hash" || "$hash" == "null" ]]; then
    echo "could not prefetch a hash for $url" >&2
    exit 1
  fi

  # Scoped to this entry rather than the first `hash =` in the file, so a
  # second architecture cannot overwrite the first one's hash. `sys`, not
  # `system`: that name is an awk builtin, and assigning it makes awk run every
  # line of the file through a shell.
  awk -v sys="$arch" -v hash="$hash" '
    $0 ~ "^    " sys " = \\{" { inside = 1 }
    inside && /^      hash = / {
      sub(/"[^"]*"/, "\"" hash "\"")
      inside = 0
    }
    { print }
  ' "$draft" >"$draft.new"
  mv "$draft.new" "$draft"

  if ! grep -q "hash = \"$hash\";" "$draft"; then
    echo "failed to write the $arch hash into $(basename "$FILE")" >&2
    exit 1
  fi
  echo "  $arch = $hash"
done

sed -i.bak "s|^  version = \".*\";$|  version = \"$VERSION\";|" "$draft"
rm -f "$draft.bak"

if ! grep -q "^  version = \"$VERSION\";$" "$draft"; then
  echo "failed to write the version into $(basename "$FILE")" >&2
  exit 1
fi

cp "$draft" "$FILE"
echo "packaging/nix/package-darwin.nix now pins $VERSION"

if ((${#unpackaged[@]})); then
  printf 'release asset with no sources entry: %s\n' "${unpackaged[@]}" >&2
  exit 3
fi
