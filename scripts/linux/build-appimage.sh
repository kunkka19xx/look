#!/usr/bin/env bash
# Build a portable Linux AppImage in an ubuntu-22.04 container, mirroring
# .github/workflows/release-linux.yml so local builds match release builds.
# Works on any docker host, including NixOS where linuxdeploy cannot run
# directly.
#
# Output: dist/Look_<version>_amd64.AppImage at the repo root.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE=look-appimage-builder

docker build -t "$IMAGE" - <<'EOF_IMAGE'
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libxcb-xkb-dev \
    libwayland-dev libasound2-dev libdbus-1-dev libssl-dev \
    libfontconfig1-dev libfreetype-dev \
    pkg-config curl ca-certificates build-essential file wget xdg-utils \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
EOF_IMAGE

docker run --rm \
  -v "$REPO_ROOT":/work \
  -v look-appimage-target:/target \
  -v look-appimage-registry:/root/.cargo/registry \
  -v look-appimage-cache:/root/.cache \
  -e CARGO_TARGET_DIR=/target \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -w /work/apps/linows \
  "$IMAGE" bash -euc '
    cargo build --release --locked -p lookapp
    VERSION=$(/target/release/lookapp --version | awk "{print \$2}")
    mkdir -p /work/dist
    /work/scripts/linux/make-appimage.sh /target/release/lookapp "$VERSION" \
      "/work/dist/Look_${VERSION}_amd64.AppImage"
    chown -R "$HOST_UID:$HOST_GID" /work/dist
  '
