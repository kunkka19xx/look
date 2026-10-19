#!/usr/bin/env bash
# Enforce the dependency direction of the core workspace. Two rules:
#
#   1. a core crate depends only on its siblings in core/, never on a shell or
#      the FFI bridge, so any crate stays extractable without a UI behind it.
#   2. nothing in core/ depends on look-ai: the AI surface is a leaf feature.
#
# Run from anywhere; exits non-zero naming every violation.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failures=0

for manifest in "$root"/core/*/Cargo.toml; do
  crate="$(basename "$(dirname "$manifest")")"

  while IFS= read -r dep_path; do
    case "$dep_path" in
      ../*/*|/*|*apps/*|*bridge/*|*tools/perf*)
        echo "error: core/$crate depends on '$dep_path', which is outside core/"
        failures=$((failures + 1))
        ;;
      ../*) ;;
      *)
        echo "error: core/$crate has an unexpected path dependency '$dep_path'"
        failures=$((failures + 1))
        ;;
    esac
  done < <(grep -oE "path[[:space:]]*=[[:space:]]*[\"'][^\"']*[\"']" "$manifest" \
    | sed -E "s/path[[:space:]]*=[[:space:]]*[\"']//; s/[\"']\$//")

  # Anywhere in the manifest, not just at the start of a line: `look-ai` also
  # arrives as `[dependencies.look-ai]` and as `x = { package = "look-ai" }`.
  if [ "$crate" != "ai" ] && grep -qE '(^|[^a-z-])look-ai([^a-z-]|$)' "$manifest"; then
    echo "error: core/$crate depends on look-ai; the AI surface is a leaf feature"
    failures=$((failures + 1))
  fi
done

if [ "$failures" -ne 0 ]; then
  echo
  echo "$failures violation(s). See the header of $0 for why these rules exist."
  exit 1
fi

echo "core dependency direction ok ($(ls "$root"/core/*/Cargo.toml | wc -l | tr -d ' ') crates checked)"
