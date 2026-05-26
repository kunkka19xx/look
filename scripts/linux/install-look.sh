#!/usr/bin/env bash
set -euo pipefail

REPO="${LOOK_REPO:-kunkka19xx/look}"
VERSION="${LOOK_VERSION:-}"
UNINSTALL=false

usage() {
  cat <<EOF
Usage: install-look.sh [OPTIONS]

Options:
  --version <x.y.z>   Install a specific version (default: latest)
  --repo <owner/repo>  GitHub repository (default: kunkka19xx/look)
  --uninstall          Remove Look and its .deb package
  -h, --help           Show this help
EOF
}

resolve_latest_version() {
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  curl -fsSL "$api_url" | grep '"tag_name"' | sed 's/.*"v\?\([^"]*\)".*/\1/'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)     REPO="$2"; shift 2 ;;
    --version)  VERSION="$2"; shift 2 ;;
    --uninstall) UNINSTALL=true; shift ;;
    -h|--help)  usage; exit 0 ;;
    *)          echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This installer is for Linux only." >&2
  echo "macOS: use scripts/install-look.sh" >&2
  echo "Windows: use scripts/windows/install-look.ps1" >&2
  exit 1
fi

if ! command -v dpkg >/dev/null 2>&1; then
  echo "dpkg not found. This installer is for Debian/Ubuntu-based distros." >&2
  echo "Arch Linux: use 'yay -S look-bin' or 'paru -S look-bin'" >&2
  echo "Other distros: download the .AppImage from https://github.com/${REPO}/releases" >&2
  exit 1
fi

# --- Uninstall ---
if [[ "$UNINSTALL" == true ]]; then
  echo "Removing Look..."
  if dpkg -s lookapp >/dev/null 2>&1; then
    sudo dpkg -r lookapp
    echo "Look has been uninstalled."
  else
    echo "Look is not installed via dpkg."
  fi
  echo ""
  echo "To remove local state (optional):"
  echo "  rm -f ~/.look.config"
  echo "  rm -rf ~/.local/share/look"
  exit 0
fi

# --- Install ---
if [[ -z "$VERSION" ]]; then
  VERSION="$(resolve_latest_version || true)"
  if [[ -z "$VERSION" ]]; then
    echo "Unable to resolve latest version from GitHub." >&2
    echo "Set LOOK_VERSION or pass --version <x.y.z>." >&2
    exit 1
  fi
fi

echo "Installing Look v${VERSION}..."

DEB_NAME="Look_${VERSION}_amd64.deb"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/${DEB_NAME}"

TMP_DIR="$(mktemp -d)"
DEB_PATH="${TMP_DIR}/${DEB_NAME}"

cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

echo "Downloading: ${DOWNLOAD_URL}"
if ! curl -fL "$DOWNLOAD_URL" -o "$DEB_PATH"; then
  echo "Download failed. Check that version '${VERSION}' exists at:" >&2
  echo "  https://github.com/${REPO}/releases" >&2
  exit 1
fi

# Remove previous version if installed
if dpkg -s lookapp >/dev/null 2>&1; then
  echo "Removing previous version..."
  sudo dpkg -r lookapp
fi

echo "Installing ${DEB_NAME}..."
sudo dpkg -i "$DEB_PATH"

# Fix missing dependencies if any
if ! dpkg -s lookapp >/dev/null 2>&1; then
  echo "Fixing dependencies..."
  sudo apt-get install -f -y
fi

echo ""
echo "Look v${VERSION} installed successfully!"
echo ""
echo "Launch:"
echo "  lookapp              # from terminal"
echo "  Alt+Space            # global hotkey (after first launch)"
echo ""
echo "Look autostarts on login. To uninstall:"
echo "  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/scripts/linux/install-look.sh | bash -s -- --uninstall"
