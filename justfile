default:
    @just --list

# Build release binary
build:
    cargo build --release

# Build and install to /usr/local/bin
install:
    #!/usr/bin/env bash
    set -euo pipefail

    # Check for required dependencies
    echo "Checking dependencies..."

    if ! command -v cargo &> /dev/null; then
        echo "❌ cargo not found"
        echo ""
        echo "Install Rust and cargo from: https://rustup.rs/"
        echo "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    echo "✓ cargo found: $(cargo --version)"

    if ! command -v rustc &> /dev/null; then
        echo "❌ rustc not found"
        echo ""
        echo "Install Rust from: https://rustup.rs/"
        exit 1
    fi
    echo "✓ rustc found: $(rustc --version)"

    echo ""
    echo "Building release binary..."
    cargo build --release

    BINARY_SRC="$(pwd)/target/release/todo"
    INSTALL_DIR="/usr/local/bin"
    BINARY_DST="$INSTALL_DIR/todo"

    if [ ! -f "$BINARY_SRC" ]; then
        echo "❌ Build failed: $BINARY_SRC not found"
        exit 1
    fi

    echo "✓ Binary built successfully"
    echo ""
    echo "Installing to $BINARY_DST..."

    # Check if files are identical
    if [ -f "$BINARY_DST" ] && cmp -s "$BINARY_SRC" "$BINARY_DST"; then
        echo "✓ Binary already installed and up to date"
    else
        # Check if we need sudo (try without first)
        if [ -w "$INSTALL_DIR" ]; then
            cp "$BINARY_SRC" "$BINARY_DST"
            chmod +x "$BINARY_DST"
        else
            echo "Need sudo to write to $INSTALL_DIR"
            sudo cp "$BINARY_SRC" "$BINARY_DST"
            sudo chmod +x "$BINARY_DST"
        fi
        echo "✓ Installed to $BINARY_DST"
    fi
    echo ""
    echo "Run 'todo' to start the TUI"

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
    cargo run --release -- serve start --port {{ port }} --daemon

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

# Configure todo-mcp for Claude CLI
setup-mcp-claude:
    #!/usr/bin/env bash
    set -euo pipefail

    # Check if claude CLI is available
    if ! command -v claude &> /dev/null; then
        echo "❌ claude CLI not found"
        echo ""
        echo "Install Claude CLI from: https://docs.anthropic.com/claude/docs/claude-cli"
        exit 1
    fi
    echo "✓ claude CLI found"

    # Build release binary
    echo ""
    echo "Building MCP server binary..."
    cargo build --release --bin todo-mcp

    BINARY_PATH="$(pwd)/target/release/todo-mcp"

    if [ ! -f "$BINARY_PATH" ]; then
        echo "❌ Build failed: $BINARY_PATH not found"
        exit 1
    fi

    echo "✓ Binary built successfully"
    echo ""
    echo "Adding todo-mcp to Claude CLI..."

    if claude mcp add todo-mcp "$BINARY_PATH" 2>&1 | grep -q "already exists"; then
        echo "✓ todo-mcp already configured, updating..."
        claude mcp remove todo-mcp || true
        claude mcp add todo-mcp "$BINARY_PATH"
    fi

    echo ""
    echo "✓ todo-mcp configured for Claude CLI"
    echo "  Binary: $BINARY_PATH"

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
release-patch msg="": (_release "patch" msg)

# Bump minor version (0.1.0 → 0.2.0)
release-minor msg="": (_release "minor" msg)

# Bump major version (0.1.0 → 1.0.0)
release-major msg="": (_release "major" msg)

_release bump msg="":
    #!/usr/bin/env bash
    set -euo pipefail

    VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"

    case "{{ bump }}" in
        patch) PATCH=$((PATCH + 1)) ;;
        minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
        major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    esac

    NEW_VERSION="$MAJOR.$MINOR.$PATCH"
    sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
    echo "✓ Version: $VERSION → $NEW_VERSION"

    # Update Cargo.lock with new version
    cargo check --quiet

    read -p "Create commit and tag? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        git add Cargo.toml Cargo.lock
        if [ -n "{{ msg }}" ]; then
            git commit -m "Release v$NEW_VERSION" -m "{{ msg }}"
        else
            git commit -m "Release v$NEW_VERSION"
        fi
        git tag "v$NEW_VERSION"
        echo "✓ Created commit and tag v$NEW_VERSION"
    fi
