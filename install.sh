#!/usr/bin/env bash
# Orchestra installer — downloads the pre-built binary for your platform.
#
# Stable (default):
#   curl -fsSL https://raw.githubusercontent.com/Chris-Miracle/orch/main/install.sh | sh
#
# Beta (pre-release):
#   curl -fsSL https://raw.githubusercontent.com/Chris-Miracle/orch/main/install.sh | sh -s -- --beta

set -euo pipefail

REPO="Chris-Miracle/orch"
BIN_NAME="orchestra"
INSTALL_DIR="${ORCHESTRA_INSTALL_DIR:-$HOME/.local/bin}"
CHANNEL="stable"

# Parse flags
for arg in "$@"; do
  case "$arg" in
    --beta)   CHANNEL="beta" ;;
    --stable) CHANNEL="stable" ;;
  esac
done

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
  *)
    echo "Orchestra currently supports macOS only."
    echo "Download manually from: https://github.com/$REPO/releases/latest"
    exit 1 ;;
esac

# Resolve the download URL based on channel
echo "Channel: $CHANNEL"
if [ "$CHANNEL" = "beta" ]; then
  echo "Fetching latest beta release info..."
  TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=20" \
    | python3 -c "
import sys, json
releases = json.load(sys.stdin)
betas = [r for r in releases if r.get('prerelease', False)]
print(betas[0]['tag_name'] if betas else '')
")
  if [ -z "$TAG" ]; then
    echo "Error: no beta pre-release found. Try installing the stable release instead."
    exit 1
  fi
  URL="https://github.com/$REPO/releases/download/$TAG/$ASSET"
  echo "Installing $TAG..."
else
  URL="https://github.com/$REPO/releases/latest/download/$ASSET"
  TAG="latest"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ASSET..."
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

# Remove macOS quarantine flag
xattr -d com.apple.quarantine "$INSTALL_DIR/$BIN_NAME" 2>/dev/null || true

# Write release channel so `orchestra update` knows which channel to use
ORCHESTRA_HOME="$HOME/.orchestra"
mkdir -p "$ORCHESTRA_HOME"
echo "$CHANNEL" > "$ORCHESTRA_HOME/channel"

echo ""
echo "✓ Installed orchestra to $INSTALL_DIR/$BIN_NAME"
echo "✓ Release channel: $CHANNEL"
echo ""

if ! echo ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
  echo "Add to your shell profile:"
  echo ""
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
  echo ""
fi

echo "Run: orchestra --help"
echo "     orchestra update          # auto-upgrade to latest $CHANNEL release"