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
TARGET_VERSION="${LATEST_TAG#v}"
echo ""

# Check installed versions of all binaries
get_installed_version() {
    local binary_name="$1"
    local existing_bin
    existing_bin=$(command -v "$binary_name" 2>/dev/null || true)
    if [ -n "$existing_bin" ]; then
        "$existing_bin" --version 2>/dev/null | awk '{print $2}' || true
    fi
}

TOTUI_VERSION=$(get_installed_version "totui")
TOTUI_MCP_VERSION=$(get_installed_version "totui-mcp")

# Show current versions if installed
if [ -n "$TOTUI_VERSION" ] || [ -n "$TOTUI_MCP_VERSION" ]; then
    echo -e "${DIM}Currently installed:${RESET}"
    if [ -n "$TOTUI_VERSION" ]; then
        if [ "$TOTUI_VERSION" = "$TARGET_VERSION" ]; then
            echo -e "  totui: ${GREEN}v${TOTUI_VERSION}${RESET} ${DIM}(up to date)${RESET}"
        else
            echo -e "  totui: ${YELLOW}v${TOTUI_VERSION}${RESET} ${DIM}(update available)${RESET}"
        fi
    fi
    if [ -n "$TOTUI_MCP_VERSION" ]; then
        if [ "$TOTUI_MCP_VERSION" = "$TARGET_VERSION" ]; then
            echo -e "  totui-mcp: ${GREEN}v${TOTUI_MCP_VERSION}${RESET} ${DIM}(up to date)${RESET}"
        else
            echo -e "  totui-mcp: ${YELLOW}v${TOTUI_MCP_VERSION}${RESET} ${DIM}(update available)${RESET}"
        fi
    fi
    echo ""
fi

# Check if all binaries are already up to date
if [ "$TOTUI_VERSION" = "$TARGET_VERSION" ] && [ "$TOTUI_MCP_VERSION" = "$TARGET_VERSION" ]; then
    echo -e "${GREEN}${BOLD}✓${RESET} All binaries are already up to date!"
    echo ""
    echo -e "Documentation: ${BLUE}https://github.com/${REPO}${RESET}"
    echo ""
    exit 0
fi

# Ask what to install
echo -e "${BOLD}What would you like to install?${RESET}"
echo ""

# Build menu options based on what needs updating
if [ "$TOTUI_VERSION" = "$TARGET_VERSION" ]; then
    echo -e "  ${DIM}1) totui only (already up to date)${RESET}"
else
    echo -e "  ${CYAN}1)${RESET} totui only ${DIM}(TUI app)${RESET}"
fi

if [ "$TOTUI_MCP_VERSION" = "$TARGET_VERSION" ]; then
    echo -e "  ${DIM}2) totui-mcp only (already up to date)${RESET}"
else
    echo -e "  ${CYAN}2)${RESET} totui-mcp only ${DIM}(MCP server for Claude/LLMs)${RESET}"
fi

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

# Filter out already up-to-date binaries
BINARIES_TO_INSTALL=()
for BINARY_NAME in "${BINARIES[@]}"; do
    if [ "$BINARY_NAME" = "totui" ] && [ "$TOTUI_VERSION" = "$TARGET_VERSION" ]; then
        echo -e "${GREEN}${BOLD}✓${RESET} ${BOLD}totui${RESET} ${DIM}v${TARGET_VERSION}${RESET} already installed ${DIM}(skipping)${RESET}"
    elif [ "$BINARY_NAME" = "totui-mcp" ] && [ "$TOTUI_MCP_VERSION" = "$TARGET_VERSION" ]; then
        echo -e "${GREEN}${BOLD}✓${RESET} ${BOLD}totui-mcp${RESET} ${DIM}v${TARGET_VERSION}${RESET} already installed ${DIM}(skipping)${RESET}"
    else
        BINARIES_TO_INSTALL+=("$BINARY_NAME")
    fi
done

if [ ${#BINARIES_TO_INSTALL[@]} -eq 0 ]; then
    echo ""
    echo -e "${GREEN}${BOLD}✓${RESET} Nothing to install - all selected binaries are up to date!"
    echo ""
    exit 0
fi

BINARIES=("${BINARIES_TO_INSTALL[@]}")
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

# Migrate data from .todo-cli to .to-tui if needed
LEGACY_DIR="$HOME/.todo-cli"
NEW_DIR="$HOME/.to-tui"

if [ -d "$LEGACY_DIR" ] && [ ! -f "$NEW_DIR/todos.db" ]; then
    # Check if legacy dir has data worth migrating
    if [ -f "$LEGACY_DIR/todos.db" ] || [ -d "$LEGACY_DIR/dailies" ]; then
        echo -e "${BLUE}→${RESET} Migrating data from ${DIM}$LEGACY_DIR${RESET} to ${DIM}$NEW_DIR${RESET}..."

        # Create new directory
        mkdir -p "$NEW_DIR"

        # Migrate database
        if [ -f "$LEGACY_DIR/todos.db" ]; then
            cp "$LEGACY_DIR/todos.db" "$NEW_DIR/todos.db"
            echo -e "  ${GREEN}✓${RESET} Migrated database: ${DIM}todos.db${RESET}"
        fi

        # Migrate dailies directory
        if [ -d "$LEGACY_DIR/dailies" ]; then
            cp -r "$LEGACY_DIR/dailies" "$NEW_DIR/dailies"
            echo -e "  ${GREEN}✓${RESET} Migrated dailies directory"
        fi

        # Migrate config
        if [ -f "$LEGACY_DIR/config.toml" ]; then
            cp "$LEGACY_DIR/config.toml" "$NEW_DIR/config.toml"
            echo -e "  ${GREEN}✓${RESET} Migrated config: ${DIM}config.toml${RESET}"
        fi

        echo ""
        echo -e "${GREEN}${BOLD}✓${RESET} Migration complete!"
        echo -e "  ${DIM}You can safely remove $LEGACY_DIR once you verify everything works.${RESET}"
        echo ""
    fi
fi

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
