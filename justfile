default:
    @just --list

# Build release binary
build:
    cargo build --release

# Build and install to ~/.local/bin
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

    # Get current version and create dev version with timestamp
    ORIGINAL_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    DEV_TIMESTAMP=$(date +%Y%m%d-%H%M%S)
    DEV_VERSION="${ORIGINAL_VERSION}-dev-${DEV_TIMESTAMP}"

    echo "Building dev version: v${DEV_VERSION}"
    echo ""

    # Temporarily modify Cargo.toml with dev version
    sed -i '' "s/^version = \".*\"/version = \"$DEV_VERSION\"/" Cargo.toml

    # Ensure we restore the original version even if build fails
    cleanup() {
        sed -i '' "s/^version = \".*\"/version = \"$ORIGINAL_VERSION\"/" Cargo.toml
    }
    trap cleanup EXIT

    echo "Building release binaries..."
    cargo build --release

    INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
    BINARIES=("totui" "totui-mcp")

    # Check for existing installations in different locations
    check_existing_binary() {
        local binary_name="$1"
        local existing_path
        existing_path=$(command -v "$binary_name" 2>/dev/null || true)

        if [ -n "$existing_path" ]; then
            existing_path=$(realpath "$existing_path" 2>/dev/null || echo "$existing_path")
            local existing_dir=$(dirname "$existing_path")

            if [ "$existing_dir" != "$INSTALL_DIR" ]; then
                echo "$existing_path"
            fi
        fi
    }

    EXISTING_BINARIES=()
    for BINARY_NAME in "${BINARIES[@]}"; do
        if [ -f "$(pwd)/target/release/$BINARY_NAME" ]; then
            EXISTING=$(check_existing_binary "$BINARY_NAME")
            if [ -n "$EXISTING" ]; then
                EXISTING_BINARIES+=("$BINARY_NAME:$EXISTING")
            fi
        fi
    done

    if [ ${#EXISTING_BINARIES[@]} -gt 0 ]; then
        echo ""
        echo "⚠️  Found existing installation(s) in different location:"
        echo ""
        for entry in "${EXISTING_BINARIES[@]}"; do
            binary_name="${entry%%:*}"
            existing_path="${entry#*:}"
            echo "   $binary_name: $existing_path"
        done
        echo ""
        echo "New install directory: $INSTALL_DIR"
        echo ""
        echo "What would you like to do?"
        echo ""
        echo "  1) Delete old binary and install to $INSTALL_DIR (default)"
        echo "  2) Install to existing location instead ($(dirname "${EXISTING_BINARIES[0]#*:}"))"
        echo "  3) Keep both (install to $INSTALL_DIR anyway)"
        echo "  4) Cancel installation"
        echo ""
        read -p "Choose [1/2/3/4] (default: 1): " -n 1 -r EXISTING_CHOICE
        echo ""
        echo ""

        case "$EXISTING_CHOICE" in
            2)
                INSTALL_DIR=$(dirname "${EXISTING_BINARIES[0]#*:}")
                echo "Installing to existing location: $INSTALL_DIR"
                ;;
            3)
                echo "Installing to $INSTALL_DIR (keeping existing binaries)"
                ;;
            4)
                echo "Installation cancelled."
                exit 0
                ;;
            *)
                for entry in "${EXISTING_BINARIES[@]}"; do
                    existing_path="${entry#*:}"
                    existing_dir=$(dirname "$existing_path")
                    echo "Removing old binary: $existing_path"
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
        echo "Note: Will need sudo to install to $INSTALL_DIR"
        echo ""
    fi

    # Install each binary
    for BINARY_NAME in "${BINARIES[@]}"; do
        BINARY_SRC="$(pwd)/target/release/$BINARY_NAME"
        BINARY_DST="$INSTALL_DIR/$BINARY_NAME"

        if [ ! -f "$BINARY_SRC" ]; then
            echo "⚠️  Skipping $BINARY_NAME: not built"
            continue
        fi

        if [ -f "$BINARY_DST" ] && cmp -s "$BINARY_SRC" "$BINARY_DST"; then
            echo "✓ $BINARY_NAME already installed and up to date"
        else
            if [ "$NEED_SUDO" = true ]; then
                sudo cp "$BINARY_SRC" "$BINARY_DST"
                sudo chmod +x "$BINARY_DST"
            else
                cp "$BINARY_SRC" "$BINARY_DST"
                chmod +x "$BINARY_DST"
            fi
            echo "✓ Installed $BINARY_NAME to $BINARY_DST"
        fi
    done

    echo ""

    # Check if install dir is in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo "⚠️  $INSTALL_DIR is not in your PATH"
        echo ""
        echo "Add it to your shell config:"
        echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
        echo "  # or for zsh:"
        echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
        echo ""
        echo "Then restart your terminal or run: source ~/.bashrc (or ~/.zshrc)"
        echo ""
    fi

    echo "Run 'totui' to start the TUI"

# Run all tests
test:
    cargo test

# Start MCP server (release mode)
start-mcp-server:
    cargo run --release --bin totui-mcp

# Start MCP server with debug logging
start-mcp-server-debug:
    RUST_LOG=debug cargo run --bin totui-mcp

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
    npx @modelcontextprotocol/inspector cargo run --release --bin totui-mcp

# Run the TUI app
tui:
    cargo run --release

# Setup MCP for local Claude Code development
setup-mcp-claude-dev:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "Setting up totui-mcp for local Claude Code development..."
    echo ""

    # Build the binary
    echo "Building MCP server binary..."
    cargo build --release --bin totui-mcp

    if [ ! -f "target/release/totui-mcp" ]; then
        echo "❌ Build failed"
        exit 1
    fi

    echo "✓ Binary built successfully"
    echo ""

    # Create symlink in .claude/plugins/repos for local development
    PLUGIN_DIR="$HOME/.claude/plugins/repos/totui-mcp"
    PROJECT_DIR="$(pwd)"

    if [ -L "$PLUGIN_DIR" ]; then
        echo "✓ Symlink already exists: $PLUGIN_DIR -> $(readlink $PLUGIN_DIR)"
    else
        mkdir -p "$HOME/.claude/plugins/repos"
        ln -s "$PROJECT_DIR" "$PLUGIN_DIR"
        echo "✓ Created symlink: $PLUGIN_DIR -> $PROJECT_DIR"
    fi

    echo ""
    echo "✓ Local development setup complete"
    echo ""
    echo "Restart Claude Code to load the plugin."
    echo ""
    echo "For production use, install via GitHub URL in Claude Code:"
    echo "  /plugin -> Add from URL -> https://github.com/grimurjonsson/to-tui.git"

# Add totui-mcp to OpenCode config
configure-mcp-opencode:
    #!/usr/bin/env bash
    set -euo pipefail

    # Build release binary first
    cargo build --release --bin totui-mcp

    BINARY_PATH="$(pwd)/target/release/totui-mcp"
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
            # mcp section exists - add/update totui-mcp entry
            jq --argjson mcp "$MCP_CONFIG" '.mcp["totui-mcp"] = $mcp' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
        else
            # no mcp section - add it
            jq --argjson mcp "$MCP_CONFIG" '. + {mcp: {"totui-mcp": $mcp}}' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
        fi
        mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"
        echo "✓ Updated $CONFIG_FILE with totui-mcp server"
    else
        # Create new config file
        cat > "$CONFIG_FILE" <<EOF
    {
      "\$schema": "https://opencode.ai/config.json",
      "mcp": {
        "totui-mcp": $MCP_CONFIG
      }
    }
    EOF
        echo "✓ Created $CONFIG_FILE with totui-mcp server"
    fi

    echo ""
    echo "MCP server configured:"
    echo "  Binary: $BINARY_PATH"
    echo ""
    echo "Restart OpenCode to load the new MCP server."

# Remove totui-mcp from OpenCode config
remove-mcp-opencode:
    #!/usr/bin/env bash
    set -euo pipefail

    CONFIG_FILE="$HOME/.config/opencode/opencode.json"

    if [ -f "$CONFIG_FILE" ] && jq -e '.mcp["totui-mcp"]' "$CONFIG_FILE" > /dev/null 2>&1; then
        jq 'del(.mcp["totui-mcp"])' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
        mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"
        echo "✓ Removed totui-mcp from $CONFIG_FILE"
    else
        echo "totui-mcp not found in OpenCode config"
    fi

# Install totui-mcp skill to Claude Code
install-claude-skill:
    #!/usr/bin/env bash
    set -euo pipefail

    SKILL_NAME="totui-mcp"
    SOURCE_DIR="$(pwd)/skills/$SKILL_NAME"
    TARGET_DIR="$HOME/.claude/skills/$SKILL_NAME"

    if [ ! -d "$SOURCE_DIR" ]; then
        echo "Error: Source skill directory not found: $SOURCE_DIR"
        exit 1
    fi

    mkdir -p "$TARGET_DIR"
    cp -r "$SOURCE_DIR"/* "$TARGET_DIR/"

    echo "✓ Installed $SKILL_NAME skill to $TARGET_DIR"

# Install totui-mcp skill to OpenCode
install-opencode-skill:
    #!/usr/bin/env bash
    set -euo pipefail

    SKILL_NAME="totui-mcp"
    SOURCE_DIR="$(pwd)/skills/$SKILL_NAME"
    TARGET_DIR="$HOME/.config/opencode/skill/$SKILL_NAME"

    if [ ! -d "$SOURCE_DIR" ]; then
        echo "Error: Source skill directory not found: $SOURCE_DIR"
        exit 1
    fi

    mkdir -p "$TARGET_DIR"
    cp -r "$SOURCE_DIR"/* "$TARGET_DIR/"

    echo "✓ Installed $SKILL_NAME skill to $TARGET_DIR"

# Build release binaries for all platforms (requires cross)
build-release-binaries:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "Building release binaries for multiple platforms..."
    echo ""

    # Check if cross is installed
    if ! command -v cross &> /dev/null; then
        echo "❌ 'cross' is not installed"
        echo ""
        echo "Install cross with: cargo install cross"
        exit 1
    fi

    TARGETS=(
        "x86_64-unknown-linux-gnu"
        "aarch64-unknown-linux-gnu"
        "x86_64-apple-darwin"
        "aarch64-apple-darwin"
        "x86_64-pc-windows-gnu"
    )

    # Add targets if not already installed
    echo "Ensuring all targets are installed..."
    for target in "${TARGETS[@]}"; do
        rustup target add "$target" 2>/dev/null || true
    done
    echo ""

    mkdir -p release-binaries

    for target in "${TARGETS[@]}"; do
        echo "Building for $target..."

        # Use cargo for Apple targets (cross doesn't support them well)
        if [[ "$target" == *"apple-darwin"* ]]; then
            cargo build --release --target "$target"
            binary_ext=""
        elif [[ "$target" == *"windows"* ]]; then
            cross build --release --target "$target"
            binary_ext=".exe"
        else
            cross build --release --target "$target"
            binary_ext=""
        fi

        # Copy both binaries to release-binaries directory with target suffix
        cp "target/$target/release/totui${binary_ext}" "release-binaries/totui-$target${binary_ext}"
        cp "target/$target/release/totui-mcp${binary_ext}" "release-binaries/totui-mcp-$target${binary_ext}"
        echo "✓ Built: release-binaries/totui-$target${binary_ext}"
        echo "✓ Built: release-binaries/totui-mcp-$target${binary_ext}"
        echo ""
    done

    echo "✓ All binaries built successfully"
    echo ""
    echo "Binaries are in the release-binaries/ directory:"
    ls -lh release-binaries/
    echo ""
    echo "Upload these to your GitHub release"

# Bump patch version (0.1.0 → 0.1.1)
release-patch msg="": (_release "patch" msg)

# Bump minor version (0.1.0 → 0.2.0)
release-minor msg="": (_release "minor" msg)

# Bump major version (0.1.0 → 1.0.0)
release-major msg="": (_release "major" msg)

# Test changelog generation (dry-run, prints to stdout)
generate-changelog-test:
    #!/usr/bin/env bash
    set -euo pipefail

    VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    TODAY=$(date +%Y-%m-%d)

    echo "=== Changelog Test ==="
    echo "Current version: $VERSION"
    echo "Last tag: ${LAST_TAG:-none}"
    echo ""

    # Get commit messages since last tag
    if [ -n "$LAST_TAG" ]; then
        CHANGES=$(git log "$LAST_TAG"..HEAD --pretty=format:"- %s" --no-merges | grep -v "^- Release v" || true)
    else
        CHANGES=$(git log --pretty=format:"- %s" --no-merges | grep -v "^- Release v" || true)
    fi

    echo "=== Raw commits ==="
    echo "$CHANGES"
    echo ""

    # Generate TL;DR using Claude if available
    TLDR=""
    if command -v claude &> /dev/null && [ -n "$CHANGES" ]; then
        echo "=== Generating TL;DR with Claude... ==="
        PROMPT="Write a concise TL;DR for these changelog commits. Focus only on user-facing changes. No quotes or prefix. Commits: $CHANGES"
        TLDR=$(claude -p "$PROMPT" 2>/dev/null || true)
        echo "TL;DR: $TLDR"
        echo ""
    else
        echo "=== Claude not available, skipping TL;DR ==="
        echo ""
    fi

    # Categorize changes
    ADDED=$(echo "$CHANGES" | grep -iE '^- (feat|add)' | sed 's/^- feat[:(] */- /i; s/^- add[:(] */- /i' || true)
    FIXED=$(echo "$CHANGES" | grep -iE '^- fix' | sed 's/^- fix[:(] */- /i' || true)
    CHANGED=$(echo "$CHANGES" | grep -iE '^- (refactor|change|update)' | sed 's/^- refactor[:(] */- /i; s/^- change[:(] */- /i; s/^- update[:(] */- /i' || true)

    echo "=== Generated changelog entry ==="
    echo "## [$VERSION] - $TODAY"
    if [ -n "$TLDR" ]; then
        echo "$TLDR"
        echo ""
    fi
    if [ -n "$ADDED" ]; then
        echo "### Added"
        echo "$ADDED"
        echo ""
    fi
    if [ -n "$FIXED" ]; then
        echo "### Fixed"
        echo "$FIXED"
        echo ""
    fi
    if [ -n "$CHANGED" ]; then
        echo "### Changed"
        echo "$CHANGED"
        echo ""
    fi

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
    RELEASE_BRANCH="release/v$NEW_VERSION"

    sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
    echo "✓ Cargo.toml version: $VERSION → $NEW_VERSION"

    # Update Claude Code marketplace.json versions
    MARKETPLACE_FILE=".claude-plugin/marketplace.json"
    if [ -f "$MARKETPLACE_FILE" ]; then
        sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/g" "$MARKETPLACE_FILE"
        echo "✓ marketplace.json version: $NEW_VERSION"
    fi

    # Update Claude Code plugin.json version
    PLUGIN_FILE=".claude-plugin/plugin.json"
    if [ -f "$PLUGIN_FILE" ]; then
        sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" "$PLUGIN_FILE"
        echo "✓ plugin.json version: $NEW_VERSION"
    fi

    # Update CHANGELOG.md with git changes since last tag
    CHANGELOG_FILE="CHANGELOG.md"
    if [ -f "$CHANGELOG_FILE" ]; then
        LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
        TODAY=$(date +%Y-%m-%d)

        # Get commit messages since last tag (or all if no tag)
        if [ -n "$LAST_TAG" ]; then
            CHANGES=$(git log "$LAST_TAG"..HEAD --pretty=format:"- %s" --no-merges | grep -v "^- Release v" || true)
        else
            CHANGES=$(git log --pretty=format:"- %s" --no-merges | grep -v "^- Release v" || true)
        fi

        # Generate TL;DR using Claude if available
        TLDR=""
        if command -v claude &> /dev/null && [ -n "$CHANGES" ]; then
            PROMPT="Write a concise TL;DR for these changelog commits. Focus only on user-facing changes. No quotes or prefix. Commits: $CHANGES"
            TLDR=$(claude -p "$PROMPT" 2>/dev/null || true)
        fi

        # Categorize changes (strip conventional commit prefixes)
        ADDED=$(echo "$CHANGES" | grep -iE '^- (feat|add)' | sed 's/^- feat[:(] */- /i; s/^- add[:(] */- /i' || true)
        FIXED=$(echo "$CHANGES" | grep -iE '^- fix' | sed 's/^- fix[:(] */- /i' || true)
        CHANGED=$(echo "$CHANGES" | grep -iE '^- (refactor|change|update)' | sed 's/^- refactor[:(] */- /i; s/^- change[:(] */- /i; s/^- update[:(] */- /i' || true)

        # Build new changelog entry using printf to avoid justfile comment issues
        TMPFILE=$(mktemp)
        printf '%s\n' "## [$NEW_VERSION] - $TODAY" >> "$TMPFILE"

        if [ -n "$TLDR" ]; then
            printf '%s\n\n' "$TLDR" >> "$TMPFILE"
        fi

        if [ -n "$ADDED" ]; then
            printf '%s\n%s\n\n' "### Added" "$ADDED" >> "$TMPFILE"
        fi

        if [ -n "$FIXED" ]; then
            printf '%s\n%s\n\n' "### Fixed" "$FIXED" >> "$TMPFILE"
        fi

        if [ -n "$CHANGED" ]; then
            printf '%s\n%s\n\n' "### Changed" "$CHANGED" >> "$TMPFILE"
        fi

        # Insert new entry after the header, before first version entry
        HEADER_END=$(grep -n '^\#\# \[' "$CHANGELOG_FILE" | head -1 | cut -d: -f1)
        if [ -n "$HEADER_END" ]; then
            OUTFILE=$(mktemp)
            head -n $((HEADER_END - 1)) "$CHANGELOG_FILE" > "$OUTFILE"
            cat "$TMPFILE" >> "$OUTFILE"
            tail -n +$HEADER_END "$CHANGELOG_FILE" >> "$OUTFILE"
            mv "$OUTFILE" "$CHANGELOG_FILE"
            echo "Updated CHANGELOG.md with v$NEW_VERSION"
        fi
        rm -f "$TMPFILE"
    fi

    # Update Cargo.lock with new version
    cargo check --quiet

    read -p "Create release branch, commit, and tag? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        # Create release branch from current HEAD
        git checkout -b "$RELEASE_BRANCH"
        echo "✓ Created branch $RELEASE_BRANCH"

        git add Cargo.toml Cargo.lock
        if [ -f "$MARKETPLACE_FILE" ]; then
            git add "$MARKETPLACE_FILE"
        fi
        if [ -f "$PLUGIN_FILE" ]; then
            git add "$PLUGIN_FILE"
        fi
        if [ -f "$CHANGELOG_FILE" ]; then
            git add "$CHANGELOG_FILE"
        fi
        if [ -n "{{ msg }}" ]; then
            git commit -m "Release v$NEW_VERSION" -m "{{ msg }}"
        else
            git commit -m "Release v$NEW_VERSION"
        fi
        git tag "v$NEW_VERSION"
        echo "✓ Created commit and tag v$NEW_VERSION"

        read -p "Push branch and tag, then create PR? [Y/n] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Nn]$ ]]; then
            # Push release branch and tag
            git push -u origin "$RELEASE_BRANCH"
            git push origin "v$NEW_VERSION"
            echo "✓ Pushed branch and tag to origin"
            echo ""
            echo "The tag push will trigger the release workflow."
            echo ""

            # Create PR using gh CLI if available
            if command -v gh &> /dev/null; then
                read -p "Create PR to merge release branch to main? [Y/n] " -n 1 -r
                echo
                if [[ ! $REPLY =~ ^[Nn]$ ]]; then
                    PR_URL=$(gh pr create \
                        --title "Release v$NEW_VERSION" \
                        --body "Release v$NEW_VERSION

This PR merges the release commit and updates:
- Cargo.toml version bump
- CHANGELOG.md updates
- Any other version files

The release workflow has already been triggered by the tag push." \
                        --base main \
                        --head "$RELEASE_BRANCH")
                    echo "✓ Created PR: $PR_URL"
                    echo ""

                    read -p "Merge the PR now? [Y/n] " -n 1 -r
                    echo
                    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
                        gh pr merge "$RELEASE_BRANCH" --merge --delete-branch
                        echo "✓ PR merged and release branch deleted"
                        git checkout main
                        git pull origin main
                        echo "✓ Switched to main and pulled latest"
                    fi
                fi
            else
                echo "gh CLI not found. Please create a PR manually to merge $RELEASE_BRANCH to main."
            fi
        fi
    fi
