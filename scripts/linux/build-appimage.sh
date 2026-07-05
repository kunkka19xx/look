#!/usr/bin/env bash
# Build a portable Linux AppImage in an ubuntu-22.04 container, mirroring
# .github/workflows/release-linux.yml so local builds match release builds.
# Works on any docker host, including NixOS where Tauri's AppImage tooling
# cannot run directly.
#
# Output: dist/Look_<version>_amd64.AppImage at the repo root.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE=look-appimage-builder

docker build -t "$IMAGE" - <<'EOF'
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libglib2.0-dev \
    libcairo2-dev libpango1.0-dev libgdk-pixbuf-2.0-dev libharfbuzz-dev \
    libdbus-1-dev libasound2-dev librsvg2-dev libssl-dev libappindicator3-dev \
    pkg-config curl ca-certificates build-essential file wget xdg-utils \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && cargo binstall -y tauri-cli --version "^2"
# linuxdeploy is itself an AppImage and there is no FUSE inside the container
ENV APPIMAGE_EXTRACT_AND_RUN=1
EOF

docker run --rm \
  -v "$REPO_ROOT":/work \
  -v look-appimage-target:/target \
  -v look-appimage-registry:/root/.cargo/registry \
  -v look-appimage-cache:/root/.cache \
  -e CARGO_TARGET_DIR=/target \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -w /work/apps/linows \
  "$IMAGE" bash -euc '
    cargo tauri build --bundles appimage
    mkdir -p /work/dist
    cp /target/release/bundle/appimage/*.AppImage /work/dist/
    for f in /work/dist/*.AppImage; do
      /work/scripts/linux/strip-appimage-wayland-libs.sh "$f"
    done
    chown -R "$HOST_UID:$HOST_GID" /work/dist
  '

echo "AppImage written to $REPO_ROOT/dist/"
