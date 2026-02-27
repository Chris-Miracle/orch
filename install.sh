#!/usr/bin/env bash
# Orchestra installer — downloads the pre-built binary for your platform.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/chris-miracle/orch/main/install.sh | sh

set -euo pipefail

REPO="chris-miracle/orch"
BIN_NAME="orchestra"
INSTALL_DIR="${ORCHESTRA_INSTALL_DIR:-$HOME/.local/bin}"

echo "Detecting platform..."

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)  ASSET="orchestra-macos-arm64.tar.gz" ;;
      x86_64) ASSET="orchestra-macos-x86_64.tar.gz" ;;
      *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
    esac ;;
  Linux)
    case "$ARCH" in
      x86_64) ASSET="orchestra-linux-x86_64.tar.gz" ;;
      *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
    esac ;;
  *)
    echo "Unsupported OS: $OS"
    echo "On Windows: download orchestra-windows-x86_64.zip from:"
    echo "https://github.com/$REPO/releases/latest"
    exit 1 ;;
esac

URL="https://github.com/$REPO/releases/latest/download/$ASSET"

echo "Downloading $ASSET..."
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$URL" -o "$TMP/$ASSET"

echo "Extracting..."
tar -xzf "$TMP/$ASSET" -C "$TMP"

if [ ! -f "$TMP/$BIN_NAME" ]; then
  echo "Error: expected binary '$BIN_NAME' not found in archive."
  echo "Check GitHub release packaging."
  exit 1
fi

echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
install -m 755 "$TMP/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"

# Remove macOS quarantine flag (safe no-op on Linux)
if [ "$OS" = "Darwin" ]; then
  xattr -d com.apple.quarantine "$INSTALL_DIR/$BIN_NAME" 2>/dev/null || true
fi

echo ""
echo "✓ Installed orchestra to $INSTALL_DIR/$BIN_NAME"
echo ""

if ! echo ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
  echo "Add to your shell profile:"
  echo ""
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
  echo ""
fi

echo "Run: orchestra --help"