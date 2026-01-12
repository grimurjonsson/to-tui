#!/usr/bin/env bash
set -euo pipefail

# Installer script for to-tui
# Downloads pre-built binaries from GitHub releases

REPO="grimurjonsson/to-tui"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'

echo ""
echo -e "${MAGENTA}${BOLD}╭─────────────────────────────────────╮${RESET}"
echo -e "${MAGENTA}${BOLD}│${RESET}       ${CYAN}${BOLD}to-tui${RESET} installer              ${MAGENTA}${BOLD}│${RESET}"
echo -e "${MAGENTA}${BOLD}╰─────────────────────────────────────╯${RESET}"
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
        echo -e "${RED}${BOLD}✗${RESET} Unsupported architecture: ${BOLD}$ARCH${RESET}"
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
        echo -e "${RED}${BOLD}✗${RESET} Unsupported OS: ${BOLD}$OS${RESET}"
        echo -e "   Supported: macOS (darwin), Linux, Windows"
        exit 1
        ;;
esac

TARGET="${ARCH}-${PLATFORM}"
echo -e "Detected platform: ${CYAN}${BOLD}$TARGET${RESET}"
echo ""

# Get the latest release tag
echo -e "${DIM}Fetching latest release...${RESET}"
API_RESPONSE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest")
LATEST_TAG=$(echo "$API_RESPONSE" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

if [ -z "$LATEST_TAG" ]; then
    echo -e "${RED}${BOLD}✗${RESET} Could not fetch latest release"
    echo ""
    if echo "$API_RESPONSE" | grep -q '"message": "Not Found"'; then
        echo -e "   No releases found. Please check ${BLUE}https://github.com/${REPO}/releases${RESET}"
    else
        echo "   API Response:"
        echo "$API_RESPONSE" | head -5
    fi
    exit 1
fi

echo -e "Latest version: ${GREEN}${BOLD}$LATEST_TAG${RESET}"
echo ""

# Ask what to install
echo -e "${BOLD}What would you like to install?${RESET}"
echo ""
echo -e "  ${CYAN}1)${RESET} totui only ${DIM}(TUI app)${RESET}"
echo -e "  ${CYAN}2)${RESET} totui-mcp only ${DIM}(MCP server for Claude/LLMs)${RESET}"
echo -e "  ${CYAN}3)${RESET} Both totui and totui-mcp"
echo ""

# Read from /dev/tty to handle curl pipe correctly
if [ -t 0 ]; then
    read -p "Choose [1/2/3] (default: 3): " -n 1 -r CHOICE
    echo ""
else
    read -p "Choose [1/2/3] (default: 3): " -n 1 -r CHOICE </dev/tty
    echo ""
fi

case "$CHOICE" in
    1) BINARIES=("totui") ;;
    2) BINARIES=("totui-mcp") ;;
    *) BINARIES=("totui" "totui-mcp") ;;
esac

echo ""

# Check for existing installations in different locations
check_existing_binary() {
    local binary_name="$1"
    local existing_path
    existing_path=$(command -v "$binary_name" 2>/dev/null || true)

    if [ -n "$existing_path" ]; then
        # Resolve symlinks to get actual path
        existing_path=$(realpath "$existing_path" 2>/dev/null || echo "$existing_path")
        local existing_dir=$(dirname "$existing_path")

        # Check if it's in a different directory than INSTALL_DIR
        if [ "$existing_dir" != "$INSTALL_DIR" ]; then
            echo "$existing_path"
        fi
    fi
}

EXISTING_BINARIES=()
for BINARY_NAME in "${BINARIES[@]}"; do
    EXISTING=$(check_existing_binary "$BINARY_NAME")
    if [ -n "$EXISTING" ]; then
        EXISTING_BINARIES+=("$BINARY_NAME:$EXISTING")
    fi
done

if [ ${#EXISTING_BINARIES[@]} -gt 0 ]; then
    echo -e "${YELLOW}${BOLD}⚠  Found existing installation(s) in different location:${RESET}"
    echo ""
    for entry in "${EXISTING_BINARIES[@]}"; do
        binary_name="${entry%%:*}"
        existing_path="${entry#*:}"
        echo -e "   ${BOLD}$binary_name${RESET}: ${DIM}$existing_path${RESET}"
    done
    echo ""
    echo -e "New install directory: ${CYAN}$INSTALL_DIR${RESET}"
    echo ""
    echo -e "${BOLD}What would you like to do?${RESET}"
    echo ""
    echo -e "  ${CYAN}1)${RESET} Delete old binary and install to $INSTALL_DIR ${DIM}(default)${RESET}"
    echo -e "  ${CYAN}2)${RESET} Install to existing location instead ${DIM}($(dirname "${EXISTING_BINARIES[0]#*:}"))${RESET}"
    echo -e "  ${CYAN}3)${RESET} Keep both ${DIM}(install to $INSTALL_DIR anyway)${RESET}"
    echo -e "  ${CYAN}4)${RESET} Cancel installation"
    echo ""
    if [ -t 0 ]; then
        read -p "Choose [1/2/3/4] (default: 1): " -n 1 -r EXISTING_CHOICE
    else
        read -p "Choose [1/2/3/4] (default: 1): " -n 1 -r EXISTING_CHOICE </dev/tty
    fi
    echo ""
    echo ""

    case "$EXISTING_CHOICE" in
        2)
            # Change install dir to existing location
            INSTALL_DIR=$(dirname "${EXISTING_BINARIES[0]#*:}")
            echo -e "${BLUE}→${RESET} Installing to existing location: ${CYAN}$INSTALL_DIR${RESET}"
            ;;
        3)
            echo -e "${BLUE}→${RESET} Installing to ${CYAN}$INSTALL_DIR${RESET} ${DIM}(keeping existing binaries)${RESET}"
            ;;
        4)
            echo -e "${YELLOW}Installation cancelled.${RESET}"
            exit 0
            ;;
        *)
            # Default: delete old binaries
            for entry in "${EXISTING_BINARIES[@]}"; do
                existing_path="${entry#*:}"
                existing_dir=$(dirname "$existing_path")
                echo -e "${RED}→${RESET} Removing old binary: ${DIM}$existing_path${RESET}"
                if [ -w "$existing_dir" ]; then
                    rm -f "$existing_path"
                else
                    sudo rm -f "$existing_path"
                fi
            done
            echo ""
            ;;
    esac
fi

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

# Check if we need sudo
NEED_SUDO=false
if [ ! -w "$INSTALL_DIR" ]; then
    NEED_SUDO=true
    echo -e "${DIM}Note: Will need sudo to install to $INSTALL_DIR${RESET}"
    echo ""
fi

# Download and install each binary
for BINARY_NAME in "${BINARIES[@]}"; do
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${BINARY_NAME}-${TARGET}${BINARY_EXT}"

    echo -e "${BLUE}↓${RESET} Downloading ${BOLD}${BINARY_NAME}${RESET}..."

    # Create temp file
    TEMP_FILE=$(mktemp)

    if ! curl -L -f -o "$TEMP_FILE" "$DOWNLOAD_URL" 2>/dev/null; then
        echo -e "${RED}${BOLD}✗${RESET} Download failed for ${BOLD}${BINARY_NAME}${RESET}"
        echo -e "   ${DIM}URL: $DOWNLOAD_URL${RESET}"
        echo -e "   ${DIM}This might mean no binary exists for your platform ($TARGET)${RESET}"
        rm -f "$TEMP_FILE"
        continue
    fi

    chmod +x "$TEMP_FILE"

    # Install
    DEST="${INSTALL_DIR}/${BINARY_NAME}${BINARY_EXT}"
    if [ "$NEED_SUDO" = true ]; then
        sudo mv "$TEMP_FILE" "$DEST"
        sudo chmod +x "$DEST"
    else
        mv "$TEMP_FILE" "$DEST"
        chmod +x "$DEST"
    fi

    echo -e "${GREEN}${BOLD}✓${RESET} Installed ${BOLD}${BINARY_NAME}${RESET} to ${CYAN}${DEST}${RESET}"
done

echo ""
echo -e "${GREEN}${BOLD}╭─────────────────────────────────────╮${RESET}"
echo -e "${GREEN}${BOLD}│${RESET}       ${GREEN}${BOLD}Installation complete!${RESET}        ${GREEN}${BOLD}│${RESET}"
echo -e "${GREEN}${BOLD}╰─────────────────────────────────────╯${RESET}"
echo ""

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}${BOLD}⚠  $INSTALL_DIR is not in your PATH${RESET}"
    echo ""
    echo -e "Add it to your shell config:"
    echo -e "  ${DIM}echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc${RESET}"
    echo -e "  ${DIM}# or for zsh:${RESET}"
    echo -e "  ${DIM}echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc${RESET}"
    echo ""
    echo -e "Then restart your terminal or run: ${CYAN}source ~/.bashrc${RESET} (or ${CYAN}~/.zshrc${RESET})"
    echo ""
fi

if [[ " ${BINARIES[*]} " =~ " totui " ]]; then
    echo -e "Run '${CYAN}${BOLD}totui${RESET}' to start the TUI"
fi

if [[ " ${BINARIES[*]} " =~ " totui-mcp " ]]; then
    echo ""
    echo -e "To use ${BOLD}totui-mcp${RESET} with Claude Code:"
    echo -e "  Add to your MCP config: ${CYAN}${INSTALL_DIR}/totui-mcp${RESET}"
fi

echo ""
echo -e "Documentation: ${BLUE}https://github.com/${REPO}${RESET}"
echo ""
