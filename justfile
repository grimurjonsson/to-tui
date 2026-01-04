default:
    @just --list

# Build release binary
build:
    cargo build --release

# Build and install to /usr/local/bin
install: build
    sudo cp target/release/todo /usr/local/bin/todo

# Run all tests
test:
    cargo test

# Start MCP server (release mode)
start-mcp-server:
    cargo run --release --bin todo-mcp

# Start MCP server with debug logging
start-mcp-server-debug:
    RUST_LOG=debug cargo run --bin todo-mcp

# Start REST API server as daemon
start-api-server port="3000":
    cargo run --release -- serve start --port {{port}} --daemon

# Stop REST API server daemon
stop-api-server:
    cargo run -- serve stop

# Check REST API server status
api-status:
    cargo run -- serve status

# Open MCP inspector for debugging
inspect-mcp:
    npx @modelcontextprotocol/inspector cargo run --release --bin todo-mcp

# Run the TUI app
tui:
    cargo run --release

# Add todo-mcp to OpenCode config
configure-mcp-opencode:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Build release binary first
    cargo build --release --bin todo-mcp
    
    BINARY_PATH="$(pwd)/target/release/todo-mcp"
    CONFIG_DIR="$HOME/.config/opencode"
    CONFIG_FILE="$CONFIG_DIR/opencode.json"
    
    # Ensure config directory exists
    mkdir -p "$CONFIG_DIR"
    
    # MCP server config to add
    MCP_CONFIG=$(cat <<EOF
    {
      "type": "local",
      "command": ["$BINARY_PATH"],
      "enabled": true
    }
    EOF
    )
    
    if [ -f "$CONFIG_FILE" ]; then
        # File exists - merge with existing config
        if jq -e '.mcp' "$CONFIG_FILE" > /dev/null 2>&1; then
            # mcp section exists - add/update todo-mcp entry
            jq --argjson mcp "$MCP_CONFIG" '.mcp["todo-mcp"] = $mcp' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
        else
            # no mcp section - add it
            jq --argjson mcp "$MCP_CONFIG" '. + {mcp: {"todo-mcp": $mcp}}' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
        fi
        mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"
        echo "✓ Updated $CONFIG_FILE with todo-mcp server"
    else
        # Create new config file
        cat > "$CONFIG_FILE" <<EOF
    {
      "\$schema": "https://opencode.ai/config.json",
      "mcp": {
        "todo-mcp": $MCP_CONFIG
      }
    }
    EOF
        echo "✓ Created $CONFIG_FILE with todo-mcp server"
    fi
    
    echo ""
    echo "MCP server configured:"
    echo "  Binary: $BINARY_PATH"
    echo ""
    echo "Restart OpenCode to load the new MCP server."

# Remove todo-mcp from OpenCode config
remove-mcp-opencode:
    #!/usr/bin/env bash
    set -euo pipefail
    
    CONFIG_FILE="$HOME/.config/opencode/opencode.json"
    
    if [ -f "$CONFIG_FILE" ] && jq -e '.mcp["todo-mcp"]' "$CONFIG_FILE" > /dev/null 2>&1; then
        jq 'del(.mcp["todo-mcp"])' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
        mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"
        echo "✓ Removed todo-mcp from $CONFIG_FILE"
    else
        echo "todo-mcp not found in OpenCode config"
    fi

# Install todo-mcp skill to Claude Code
install-claude-skill:
    #!/usr/bin/env bash
    set -euo pipefail
    
    SKILL_NAME="todo-mcp"
    SOURCE_DIR="$(pwd)/skills/$SKILL_NAME"
    TARGET_DIR="$HOME/.claude/skills/$SKILL_NAME"
    
    if [ ! -d "$SOURCE_DIR" ]; then
        echo "Error: Source skill directory not found: $SOURCE_DIR"
        exit 1
    fi
    
    mkdir -p "$TARGET_DIR"
    cp -r "$SOURCE_DIR"/* "$TARGET_DIR/"
    
    echo "✓ Installed $SKILL_NAME skill to $TARGET_DIR"

# Install todo-mcp skill to OpenCode
install-opencode-skill:
    #!/usr/bin/env bash
    set -euo pipefail
    
    SKILL_NAME="todo-mcp"
    SOURCE_DIR="$(pwd)/skills/$SKILL_NAME"
    TARGET_DIR="$HOME/.config/opencode/skill/$SKILL_NAME"
    
    if [ ! -d "$SOURCE_DIR" ]; then
        echo "Error: Source skill directory not found: $SOURCE_DIR"
        exit 1
    fi
    
    mkdir -p "$TARGET_DIR"
    cp -r "$SOURCE_DIR"/* "$TARGET_DIR/"
    
    echo "✓ Installed $SKILL_NAME skill to $TARGET_DIR"

# Bump patch version (0.1.0 → 0.1.1)
release-patch: (_release "patch")
# Bump minor version (0.1.0 → 0.2.0)
release-minor: (_release "minor")
# Bump major version (0.1.0 → 1.0.0)
release-major: (_release "major")

_release bump:
    #!/usr/bin/env bash
    set -euo pipefail
    
    VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"
    
    case "{{bump}}" in
        patch) PATCH=$((PATCH + 1)) ;;
        minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
        major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    esac
    
    NEW_VERSION="$MAJOR.$MINOR.$PATCH"
    sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
    echo "✓ Version: $VERSION → $NEW_VERSION"
    
    read -p "Create commit and tag? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        git add Cargo.toml
        git commit -m "Release v$NEW_VERSION"
        git tag "v$NEW_VERSION"
        echo "✓ Created commit and tag v$NEW_VERSION"
    fi
