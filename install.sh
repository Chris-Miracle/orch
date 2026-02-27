#!/usr/bin/env bash
# Orchestra installer — downloads the pre-built binary for your platform.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/chris-miracle/orch/main/install.sh | sh
set -euo pipefail

REPO="chris-miracle/orch"
BIN_NAME="orchestra"
INSTALL_DIR="${ORCHESTRA_INSTALL_DIR:-$HOME/.local/bin}"

# ── Detect platform ───────────────────────────────────────────────────────────
OS=$(uname -s)
ARCH=$(uname -m)

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
    echo "On Windows: download orchestra-windows-x86_64.zip from https://github.com/$REPO/releases/latest"
    exit 1 ;;
esac

# ── Resolve latest release ───────────────────────────────────────────────────
echo "Fetching latest Orchestra release..."
TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')

if [ -z "$TAG" ]; then
  echo "Could not determine latest release. Check https://github.com/$REPO/releases"
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$TAG/$ASSET"

# ── Download and install ──────────────────────────────────────────────────────
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ASSET ($TAG)..."
curl -fsSL "$URL" -o "$TMP/$ASSET"

echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
tar -xzf "$TMP/$ASSET" -C "$TMP"
install -m 755 "$TMP/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"

# ── PATH hint ────────────────────────────────────────────────────────────────
echo ""
echo "✓ Installed orchestra $TAG to $INSTALL_DIR/orchestra"
echo ""

if ! echo ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
  echo "Add to your shell profile:"
  echo ""
  echo '  export PATH="$HOME/.local/bin:$PATH"'
  echo ""
fi

echo "Run: orchestra --help"
