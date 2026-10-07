# Shared release artifact naming. Sourced, not executed.
#
# Releases are single-arch: one published asset per architecture, so an Apple
# Silicon download never carries an Intel slice. arm64 keeps the historical
# artifact name, which is what the Homebrew cask, the README install URL and
# scripts/install-look.sh already point at.

ARCHS="${ARCHS:-arm64}"

# shellcheck disable=SC2034  # consumed by the scripts that source this file

case "$ARCHS" in
  arm64) ARCH_SUFFIX="" ;;
  x86_64) ARCH_SUFFIX="-x86_64" ;;
  *) echo "Unsupported ARCHS for a release build: $ARCHS (expected arm64 or x86_64)" >&2; exit 1 ;;
esac
