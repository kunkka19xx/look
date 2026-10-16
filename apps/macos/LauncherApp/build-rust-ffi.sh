#!/bin/bash
# Build bridge/ffi for the architectures Xcode is building and leave the result
# at RustBuild/liblook_ffi.a.
#
# The app links this library with -Wl,-force_load, so the Rust slices have to
# cover every arch the app binary contains.
#
# Releases are single-arch (one published asset per architecture), so the two
# paths that matter are the host arch (every Debug dev loop) and one cross arch
# (the Intel release leg). Multi-arch ARCHS still works via lipo, because an
# Xcode GUI build against "Any Mac" asks for it.
#
# Run standalone (from any cwd) to test:
#   ARCHS=x86_64 CONFIGURATION=Release \
#   PROJECT_DIR="$PWD/apps/macos/LauncherApp" ./apps/macos/LauncherApp/build-rust-ffi.sh

set -euo pipefail

PROJECT_DIR="${PROJECT_DIR:-$(cd "$(dirname "$0")" && pwd)}"
ARCHS="${ARCHS:-$(uname -m)}"

FFI_MANIFEST="$PROJECT_DIR/../../../bridge/ffi/Cargo.toml"
FFI_TARGET_DIR="$PROJECT_DIR/../../../bridge/ffi/target"
OUT_LIB="$PROJECT_DIR/RustBuild/liblook_ffi.a"
CARGO_LOG="$PROJECT_DIR/RustBuild/cargo-ffi-build.log"

if [ "${CONFIGURATION:-Debug}" = "Release" ]; then
  PROFILE=release
  CARGO_FLAGS=--release
else
  PROFILE=debug
  CARGO_FLAGS=""
fi

if [ -x "$HOME/.cargo/bin/cargo" ]; then
  CARGO_BIN="$HOME/.cargo/bin/cargo"
else
  CARGO_BIN=cargo
fi

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-15.0}"

# One arch and it is the host arch: build without --target so the dev loop keeps
# bridge/ffi/target/$PROFILE as its only cargo cache and needs no cross target
# installed. Anything else goes through --target per slice.
HOST_ONLY=no
if [ "$(set -- $ARCHS; echo $#)" = 1 ] && [ "$ARCHS" = "$(uname -m)" ]; then
  HOST_ONLY=yes
fi

triple_for() {
  case "$1" in
    arm64) printf 'aarch64-apple-darwin' ;;
    x86_64) printf 'x86_64-apple-darwin' ;;
    *) echo "error: unsupported arch for Rust FFI: $1" >&2; return 1 ;;
  esac
}

cargo_build() {
  local arch="$1"
  shift
  if "$CARGO_BIN" +nightly build --manifest-path "$FFI_MANIFEST" "$@" $CARGO_FLAGS 2>&1 | tee "$CARGO_LOG"; then
    return 0
  fi
  if grep -q 'may not be installed' "$CARGO_LOG" 2>/dev/null; then
    echo "error: rust target $(triple_for "$arch") is not installed on the nightly toolchain." >&2
    echo "hint: rustup target add $(triple_for "$arch") --toolchain nightly" >&2
  else
    echo "error: cargo build failed for $arch; output above." >&2
  fi
  return 1
}

mkdir -p "$PROJECT_DIR/RustBuild"

build_slices() {
  local arch triple
  local slices=()

  if [ "$HOST_ONLY" = yes ]; then
    echo "Building Rust FFI (host arch: $ARCHS)"
    cargo_build "$ARCHS" || return 1
    cp "$FFI_TARGET_DIR/$PROFILE/liblook_ffi.a" "$OUT_LIB"
    return 0
  fi

  for arch in $ARCHS; do
    triple="$(triple_for "$arch")" || return 1
    echo "Building Rust FFI slice: $triple"
    cargo_build "$arch" --target "$triple" || return 1
    slices+=("$FFI_TARGET_DIR/$triple/$PROFILE/liblook_ffi.a")
  done

  if [ "${#slices[@]}" -eq 1 ]; then
    cp "${slices[0]}" "$OUT_LIB"
  elif ! lipo -create "${slices[@]}" -output "$OUT_LIB"; then
    echo "error: lipo failed merging: ${slices[*]}" >&2
    return 1
  fi
}

if build_slices; then
  echo "Rust FFI built for: $ARCHS"
  exit 0
fi

if [ "$PROFILE" = release ] || [ "$HOST_ONLY" != yes ]; then
  # Never link a stale archive. A failed cross slice means a missing rustup
  # target rather than a flake, and reusing the old .a hides a Rust change
  # behind a green build.
  echo "error: Rust FFI build failed; refusing to reuse an existing archive" >&2
  exit 1
fi

# Debug host build only: warn and reuse so a Swift-only iteration is not blocked
# by a broken Rust toolchain.
echo "warning: Rust build skipped; using existing RustBuild/liblook_ffi.a"
if [ ! -f "$OUT_LIB" ]; then
  echo "error: missing fallback RustBuild/liblook_ffi.a" >&2
  exit 1
fi
