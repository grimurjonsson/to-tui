#!/usr/bin/env bash
set -euo pipefail

# Script to download and install pre-built totui-mcp binary from GitHub releases
# This runs after the plugin is installed from the marketplace

REPO="grimurjonsson/to-tui"
BINARY_NAME="totui-mcp"

echo "Installing totui-mcp binary..."
echo ""

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map architecture names
case "$ARCH" in
    x86_64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        echo "❌ Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# Map OS names and set binary extension
BINARY_EXT=""
case "$OS" in
    darwin)
        PLATFORM="apple-darwin"
        ;;
    linux)
        PLATFORM="unknown-linux-gnu"
        ;;
    mingw*|msys*|cygwin*)
        PLATFORM="pc-windows-gnu"
        BINARY_EXT=".exe"
        ;;
    *)
        echo "❌ Unsupported OS: $OS"
        echo "   Supported: macOS (darwin), Linux, Windows"
        exit 1
        ;;
esac

TARGET="${ARCH}-${PLATFORM}"
echo "Detected platform: $TARGET"
echo ""

# Get the latest release tag
echo "Fetching latest release..."
API_RESPONSE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest")
LATEST_TAG=$(echo "$API_RESPONSE" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

if [ -z "$LATEST_TAG" ]; then
    echo "❌ Could not fetch latest release"
    echo ""

    # Check if it's a 404 (no releases yet)
    if echo "$API_RESPONSE" | grep -q '"message": "Not Found"'; then
        echo "   No releases found for this repository."
        echo "   The maintainer needs to create a release first."
        echo ""
        echo "   Build from source instead:"
        SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        PLUGIN_ROOT="$(dirname "$SCRIPT_DIR")"
        echo "   cd $PLUGIN_ROOT && cargo build --release --bin totui-mcp"
    else
        # Show the API response for debugging
        echo "   API Response:"
        echo "$API_RESPONSE" | head -5
        echo ""
        echo "   Please check https://github.com/${REPO}/releases"
    fi
    exit 1
fi

echo "Latest version: $LATEST_TAG"
echo ""

# Determine archive extension based on platform
if [ -z "$BINARY_EXT" ]; then
    ARCHIVE_EXT=".tar.gz"
else
    ARCHIVE_EXT=".zip"
fi

# Construct download URL
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${BINARY_NAME}-${TARGET}${ARCHIVE_EXT}"

echo "Downloading from: $DOWNLOAD_URL"

# Determine installation directory (where this script is located)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ROOT="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="$PLUGIN_ROOT/target/release"

mkdir -p "$INSTALL_DIR"

# Create temp directory for download and extraction
TEMP_DIR=$(mktemp -d)
TEMP_ARCHIVE="${TEMP_DIR}/archive${ARCHIVE_EXT}"

# Download archive
if ! curl -L -f -o "$TEMP_ARCHIVE" "$DOWNLOAD_URL"; then
    echo ""
    echo "❌ Download failed"
    echo "   URL: $DOWNLOAD_URL"
    echo "   This might mean:"
    echo "   1. No binary exists for your platform ($TARGET)"
    echo "   2. The release doesn't include pre-built binaries yet"
    echo "   3. Network connection issue"
    echo ""
    echo "   You can build from source instead:"
    echo "   cd $PLUGIN_ROOT && cargo build --release --bin totui-mcp"
    rm -rf "$TEMP_DIR"
    exit 1
fi

# Extract archive
echo "Extracting archive..."
if [ -z "$BINARY_EXT" ]; then
    # Unix: extract tar.gz
    if ! tar -xzf "$TEMP_ARCHIVE" -C "$TEMP_DIR"; then
        echo ""
        echo "❌ Failed to extract archive"
        rm -rf "$TEMP_DIR"
        exit 1
    fi
else
    # Windows: extract zip
    if ! unzip -q "$TEMP_ARCHIVE" -d "$TEMP_DIR"; then
        echo ""
        echo "❌ Failed to extract archive"
        rm -rf "$TEMP_DIR"
        exit 1
    fi
fi

# Move extracted binary to install directory
EXTRACTED_BINARY="${TEMP_DIR}/${BINARY_NAME}${BINARY_EXT}"
if [ ! -f "$EXTRACTED_BINARY" ]; then
    echo ""
    echo "❌ Binary not found in archive"
    echo "   Expected: $EXTRACTED_BINARY"
    rm -rf "$TEMP_DIR"
    exit 1
fi

mv "$EXTRACTED_BINARY" "$INSTALL_DIR/$BINARY_NAME${BINARY_EXT}"

# Make it executable (not needed on Windows but doesn't hurt)
chmod +x "$INSTALL_DIR/$BINARY_NAME${BINARY_EXT}" 2>/dev/null || true

# Clean up temp directory
rm -rf "$TEMP_DIR"

echo ""
echo "✓ Binary installed successfully"
echo "  Location: $INSTALL_DIR/$BINARY_NAME${BINARY_EXT}"
echo ""
echo "Restart Claude Code to activate the MCP server."
