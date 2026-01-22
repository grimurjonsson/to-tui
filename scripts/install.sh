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

# Helper function to compare versions (returns 0 if v1 > v2)
version_gt() {
    test "$(printf '%s\n' "$@" | sort -V | head -n 1)" != "$1"
}

# Show changelog for upgrades (called before install prompt)
show_changelog() {
    local from_version="$1"
    local to_version="$2"

    # Fetch CHANGELOG.md from main branch (always has latest changelog)
    CHANGELOG_URL="https://raw.githubusercontent.com/${REPO}/main/CHANGELOG.md"
    CHANGELOG_CONTENT=$(curl -s "$CHANGELOG_URL" 2>/dev/null || true)

    if [ -z "$CHANGELOG_CONTENT" ]; then
        return
    fi

    echo -e "${CYAN}${BOLD}What's new since v${from_version}:${RESET}"
    echo ""

    # Parse changelog and show entries between versions
    local printing=false
    local found_any=false

    while IFS= read -r line; do
        # Check for version header like "## [0.2.8]" or "## [0.2.8] - 2026-01-19"
        if echo "$line" | grep -qE '^\#\# \[[0-9]+\.[0-9]+\.[0-9]+\]'; then
            version=$(echo "$line" | grep -oE '\[([0-9]+\.[0-9]+\.[0-9]+)\]' | tr -d '[]')

            # Stop if we hit the from_version
            if [ "$version" = "$from_version" ]; then
                printing=false
                break
            fi

            # Start printing if version is newer than from_version
            if version_gt "$version" "$from_version" 2>/dev/null || [ "$version" = "$to_version" ]; then
                printing=true
                found_any=true
                echo -e "${BOLD}${line}${RESET}"
            else
                printing=false
            fi
        elif [ "$printing" = true ]; then
            # Format subsection headers
            if echo "$line" | grep -qE '^\#\#\# '; then
                echo -e "${YELLOW}${line}${RESET}"
            elif [ -n "$line" ]; then
                echo "  $line"
            else
                echo ""
            fi
        fi
    done <<< "$CHANGELOG_CONTENT"

    if [ "$found_any" = true ]; then
        echo ""
    fi
}

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

    # Show changelog for upgrades before prompting
    if [ -n "$TOTUI_VERSION" ] && [ "$TOTUI_VERSION" != "$TARGET_VERSION" ]; then
        show_changelog "$TOTUI_VERSION" "$TARGET_VERSION"
    elif [ -n "$TOTUI_MCP_VERSION" ] && [ "$TOTUI_MCP_VERSION" != "$TARGET_VERSION" ]; then
        show_changelog "$TOTUI_MCP_VERSION" "$TARGET_VERSION"
    fi
fi

# Track if totui-mcp is installed (for Claude Code integration check later)
TOTUI_MCP_INSTALLED=false
if [ -n "$TOTUI_MCP_VERSION" ]; then
    TOTUI_MCP_INSTALLED=true
fi

# Check if all binaries are already up to date - skip installation but continue to Claude Code integration
ALL_UP_TO_DATE=false
if [ "$TOTUI_VERSION" = "$TARGET_VERSION" ] && [ "$TOTUI_MCP_VERSION" = "$TARGET_VERSION" ]; then
    echo -e "${GREEN}${BOLD}✓${RESET} All binaries are already up to date!"
    echo ""
    ALL_UP_TO_DATE=true
fi

if [ "$ALL_UP_TO_DATE" = false ]; then

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

if [ ${#BINARIES_TO_INSTALL[@]} -gt 0 ]; then

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
    # Determine archive extension based on platform
    if [ -z "$BINARY_EXT" ]; then
        ARCHIVE_EXT=".tar.gz"
    else
        ARCHIVE_EXT=".zip"
    fi

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${BINARY_NAME}-${TARGET}${ARCHIVE_EXT}"

    echo -e "${BLUE}↓${RESET} Downloading ${BOLD}${BINARY_NAME}${RESET}..."

    # Create temp directory for extraction
    TEMP_DIR=$(mktemp -d)
    TEMP_ARCHIVE="${TEMP_DIR}/archive${ARCHIVE_EXT}"

    if ! curl -L -f -o "$TEMP_ARCHIVE" "$DOWNLOAD_URL" 2>/dev/null; then
        echo -e "${RED}${BOLD}✗${RESET} Download failed for ${BOLD}${BINARY_NAME}${RESET}"
        echo -e "   ${DIM}URL: $DOWNLOAD_URL${RESET}"
        echo -e "   ${DIM}This might mean no binary exists for your platform ($TARGET)${RESET}"
        rm -rf "$TEMP_DIR"
        continue
    fi

    # Extract archive
    if [ -z "$BINARY_EXT" ]; then
        # Unix: extract tar.gz
        tar -xzf "$TEMP_ARCHIVE" -C "$TEMP_DIR" || {
            echo -e "${RED}${BOLD}✗${RESET} Failed to extract archive for ${BOLD}${BINARY_NAME}${RESET}"
            rm -rf "$TEMP_DIR"
            continue
        }
    else
        # Windows: extract zip
        unzip -q "$TEMP_ARCHIVE" -d "$TEMP_DIR" || {
            echo -e "${RED}${BOLD}✗${RESET} Failed to extract archive for ${BOLD}${BINARY_NAME}${RESET}"
            rm -rf "$TEMP_DIR"
            continue
        }
    fi

    # Move extracted binary to install directory
    EXTRACTED_BINARY="${TEMP_DIR}/${BINARY_NAME}${BINARY_EXT}"
    DEST="${INSTALL_DIR}/${BINARY_NAME}${BINARY_EXT}"

    if [ ! -f "$EXTRACTED_BINARY" ]; then
        echo -e "${RED}${BOLD}✗${RESET} Binary not found in archive for ${BOLD}${BINARY_NAME}${RESET}"
        rm -rf "$TEMP_DIR"
        continue
    fi

    if [ "$NEED_SUDO" = true ]; then
        sudo mv "$EXTRACTED_BINARY" "$DEST"
        sudo chmod +x "$DEST"
    else
        mv "$EXTRACTED_BINARY" "$DEST"
        chmod +x "$DEST"
    fi

    # Clean up temp directory
    rm -rf "$TEMP_DIR"

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

fi # end: if BINARIES_TO_INSTALL not empty
fi # end: if not ALL_UP_TO_DATE

# ─────────────────────────────────────────────────────────────
# Claude Code Integration (optional)
# ─────────────────────────────────────────────────────────────

# Check if totui-mcp was just installed OR was already installed
if [ "$TOTUI_MCP_INSTALLED" = true ] || [[ " ${BINARIES[*]} " =~ " totui-mcp " ]]; then
    # Check if claude CLI is available
    if command -v claude &>/dev/null; then
        echo -e "${BLUE}${BOLD}Claude Code Integration${RESET}"
        echo ""

        MARKETPLACE_REPO="grimurjonsson/to-tui"
        MARKETPLACE_NAME="totui-mcp"
        PLUGIN_ID="totui-mcp@totui-mcp"
        MARKETPLACE_AVAILABLE=false

        # Check if marketplace is already registered
        if claude plugin marketplace list 2>/dev/null | grep -q "$MARKETPLACE_NAME"; then
            echo -e "  ${GREEN}✓${RESET} Marketplace already registered"
            MARKETPLACE_AVAILABLE=true
        else
            echo -e "  ${YELLOW}→${RESET} Marketplace not registered"
            echo ""
            if [ -t 0 ]; then
                read -p "  Add totui marketplace to Claude Code? [Y/n] " -n 1 -r MARKETPLACE_CHOICE
            else
                read -p "  Add totui marketplace to Claude Code? [Y/n] " -n 1 -r MARKETPLACE_CHOICE </dev/tty
            fi
            echo ""

            if [[ ! $MARKETPLACE_CHOICE =~ ^[Nn]$ ]]; then
                if claude plugin marketplace add "$MARKETPLACE_REPO" 2>/dev/null; then
                    echo -e "  ${GREEN}✓${RESET} Marketplace added: ${CYAN}$MARKETPLACE_REPO${RESET}"
                    MARKETPLACE_AVAILABLE=true
                else
                    echo -e "  ${RED}✗${RESET} Failed to add marketplace"
                fi
            fi
        fi

        # If marketplace is available, check/install the plugin
        if [ "$MARKETPLACE_AVAILABLE" = true ]; then
            echo ""
            if claude plugin list 2>/dev/null | grep -q "$PLUGIN_ID"; then
                echo -e "  ${GREEN}✓${RESET} Plugin already installed"
            else
                echo -e "  ${YELLOW}→${RESET} Plugin not installed"
                echo ""
                if [ -t 0 ]; then
                    read -p "  Install totui-mcp plugin from marketplace? [Y/n] " -n 1 -r PLUGIN_CHOICE
                else
                    read -p "  Install totui-mcp plugin from marketplace? [Y/n] " -n 1 -r PLUGIN_CHOICE </dev/tty
                fi
                echo ""

                if [[ ! $PLUGIN_CHOICE =~ ^[Nn]$ ]]; then
                    if claude plugin install "$PLUGIN_ID" --scope user 2>/dev/null; then
                        echo -e "  ${GREEN}✓${RESET} Plugin installed: ${CYAN}$PLUGIN_ID${RESET}"
                        echo ""
                        echo -e "  ${DIM}Restart Claude Code to activate the plugin.${RESET}"
                    else
                        echo -e "  ${RED}✗${RESET} Failed to install plugin"
                        echo -e "  ${DIM}Manual setup: claude plugin install $PLUGIN_ID --scope user${RESET}"
                    fi
                fi
            fi
        else
            # Marketplace not available - offer standalone MCP server as fallback
            echo ""
            if claude mcp list 2>/dev/null | grep -q "totui-mcp"; then
                echo -e "  ${GREEN}✓${RESET} MCP server already configured"
            else
                echo -e "  ${YELLOW}→${RESET} MCP server not configured"
                echo ""
                if [ -t 0 ]; then
                    read -p "  Configure totui-mcp as standalone MCP server? [Y/n] " -n 1 -r MCP_CHOICE
                else
                    read -p "  Configure totui-mcp as standalone MCP server? [Y/n] " -n 1 -r MCP_CHOICE </dev/tty
                fi
                echo ""

                if [[ ! $MCP_CHOICE =~ ^[Nn]$ ]]; then
                    MCP_BINARY="${INSTALL_DIR}/totui-mcp${BINARY_EXT}"
                    if claude mcp add --transport stdio --scope user totui-mcp -- "$MCP_BINARY" 2>/dev/null; then
                        echo -e "  ${GREEN}✓${RESET} MCP server configured: ${CYAN}totui-mcp${RESET}"
                        echo ""
                        echo -e "  ${DIM}Restart Claude Code to activate the MCP tools.${RESET}"
                    else
                        echo -e "  ${RED}✗${RESET} Failed to configure MCP server"
                        echo -e "  ${DIM}Manual setup: claude mcp add --transport stdio --scope user totui-mcp -- $MCP_BINARY${RESET}"
                    fi
                fi
            fi
        fi
        echo ""
    fi
fi

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

# Show usage hints based on what's installed
if [ -n "$TOTUI_VERSION" ] || [[ " ${BINARIES[*]} " =~ " totui " ]]; then
    echo -e "Run '${CYAN}${BOLD}totui${RESET}' to start the TUI"
fi

echo ""
echo -e "Documentation: ${BLUE}https://github.com/${REPO}${RESET}"
echo ""
