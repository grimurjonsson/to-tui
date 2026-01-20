# Phase 4: Claude Code Plugin Configuration - Research

**Researched:** 2026-01-20
**Domain:** Claude Code Plugin System / MCP Server Configuration
**Confidence:** HIGH

## Summary

This research covers Claude Code's plugin and MCP server configuration system for making the totui-mcp server discoverable and usable within Claude Code. The current project already has partial plugin infrastructure but needs updates to follow current Anthropic documentation patterns.

The key insight is that Claude Code supports **two distinct approaches** for MCP server integration:
1. **Direct MCP configuration** - Add servers via `claude mcp add` or `.mcp.json` files (simpler, project-scoped)
2. **Plugin system** - Bundle MCP servers with skills, commands, and hooks in a distributable package (more features, marketplace support)

The current project has elements of both approaches but needs consolidation to work properly with Claude Code's plugin marketplace system.

**Primary recommendation:** Update the plugin configuration to use `${CLAUDE_PLUGIN_ROOT}` for portable binary paths, consolidate the MCP server configuration into the plugin structure, and document the binary installation workflow that downloads pre-built binaries from GitHub releases.

## Standard Stack

The established configuration patterns for Claude Code MCP servers:

### Core Configuration Files
| File | Location | Purpose | When to Use |
|------|----------|---------|-------------|
| `.mcp.json` | Plugin root | MCP server definitions for plugins | For plugin-bundled MCP servers |
| `plugin.json` | `.claude-plugin/` | Plugin manifest with metadata | Required for all plugins |
| `marketplace.json` | `.claude-plugin/` | Marketplace distribution metadata | For marketplace distribution |

### Supporting Files
| File | Purpose | When to Use |
|------|---------|-------------|
| `skills/*/SKILL.md` | Model-invokable skills | Usage instructions for Claude |
| `scripts/install-binary.sh` | Binary installation | Post-plugin-install binary download |
| `hooks/hooks.json` | Event handlers | Optional automation hooks |

### Configuration File Locations (User-level)
| Scope | File | Description |
|-------|------|-------------|
| Project-scoped | `.mcp.json` (project root) | Shared via version control, requires approval |
| Local-scoped | `~/.claude.json` (project path) | Private to user, per-project |
| User-scoped | `~/.claude.json` | Available across all projects |

**Installation:**
```bash
# No package installation needed - configuration files only
# Binary installation is handled separately via scripts/install-binary.sh
```

## Architecture Patterns

### Recommended Plugin Directory Structure
```
totui-mcp/
  .claude-plugin/
    plugin.json           # Required: plugin manifest
    marketplace.json      # For marketplace distribution
  .mcp.json              # MCP server configuration (portable paths)
  skills/
    todo-mcp/
      SKILL.md           # Usage instructions for Claude
  scripts/
    install-binary.sh    # Downloads pre-built binary from GitHub releases
  target/
    release/
      totui-mcp          # Binary location (downloaded or built)
```

### Pattern 1: Plugin MCP Server Configuration
**What:** MCP server definition using `${CLAUDE_PLUGIN_ROOT}` variable
**When to use:** When bundling MCP servers with plugins for portable distribution
**Example:**
```json
// .mcp.json at plugin root
// Source: https://code.claude.com/docs/en/plugins-reference
{
  "totui-mcp": {
    "command": "${CLAUDE_PLUGIN_ROOT}/target/release/totui-mcp",
    "args": []
  }
}
```

### Pattern 2: plugin.json Manifest
**What:** Plugin metadata defining name, version, and components
**When to use:** Required for all Claude Code plugins
**Example:**
```json
// .claude-plugin/plugin.json
// Source: https://code.claude.com/docs/en/plugins-reference
{
  "name": "totui-mcp",
  "description": "MCP server for to-tui - terminal todo list manager",
  "version": "0.2.11",
  "author": {
    "name": "to-tui"
  },
  "repository": "https://github.com/grimurjonsson/to-tui",
  "license": "MIT",
  "keywords": ["todo", "task", "mcp", "productivity"]
}
```

### Pattern 3: Marketplace Distribution
**What:** marketplace.json for plugin distribution via GitHub
**When to use:** When distributing plugins through marketplaces
**Example:**
```json
// .claude-plugin/marketplace.json
// Source: https://code.claude.com/docs/en/plugin-marketplaces
{
  "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
  "name": "totui-mcp",
  "version": "0.2.11",
  "owner": {
    "name": "to-tui"
  },
  "plugins": [
    {
      "name": "totui-mcp",
      "description": "MCP server for to-tui - terminal todo list manager",
      "version": "0.2.11",
      "source": "./",
      "category": "productivity",
      "strict": false
    }
  ]
}
```

### Anti-Patterns to Avoid
- **Hardcoded absolute paths:** Using `/Users/gimmi/...` paths in `.mcp.json` - not portable, won't work when distributed
- **Missing `${CLAUDE_PLUGIN_ROOT}`:** Required for plugin-relative paths to work after installation
- **Inline mcpServers in plugin.json without nesting:** Server configs must be under `mcpServers` key, not at root
- **Forgetting binary installation step:** Plugins with binaries need separate binary download/build instructions

## Don't Hand-Roll

Problems that have existing solutions in the Claude Code ecosystem:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| MCP server registration | Custom registration scripts | `claude mcp add` CLI | Standard discovery, scope support |
| Plugin distribution | Manual file copying | Plugin marketplace system | Versioning, updates, team sharing |
| Binary distribution | Inline compilation during install | GitHub releases + install script | No Rust toolchain required for users |
| MCP config portability | Hardcoded paths | `${CLAUDE_PLUGIN_ROOT}` variable | Works across different installations |

**Key insight:** Claude Code's plugin system handles most distribution complexity - focus on proper configuration rather than custom tooling.

## Common Pitfalls

### Pitfall 1: Hardcoded Binary Paths
**What goes wrong:** Plugin works locally but fails when installed by other users
**Why it happens:** Using absolute paths like `/Users/username/path/to/binary`
**How to avoid:** Always use `${CLAUDE_PLUGIN_ROOT}/target/release/binary-name`
**Warning signs:** Plugin works in development but not after marketplace install

### Pitfall 2: Missing Binary After Plugin Install
**What goes wrong:** MCP server fails to start because binary doesn't exist
**Why it happens:** Plugins only copy files, don't compile or download binaries
**How to avoid:** Document binary installation step; provide `scripts/install-binary.sh`
**Warning signs:** "Command not found" or "ENOENT" errors in Claude Code

### Pitfall 3: Incorrect MCP Configuration Location
**What goes wrong:** MCP servers don't appear in Claude Code
**Why it happens:** Config in wrong location or wrong format
**How to avoid:**
  - For plugins: `.mcp.json` at plugin root (not in `.claude-plugin/`)
  - Server names as root keys in `.mcp.json`
**Warning signs:** `/mcp` command shows no servers from plugin

### Pitfall 4: Plugin Caching Issues
**What goes wrong:** Changes to plugin don't take effect
**Why it happens:** Claude Code caches plugins; old version still loaded
**How to avoid:** Restart Claude Code after plugin updates; use `--plugin-dir` for development
**Warning signs:** Old behavior persists despite code changes

### Pitfall 5: Scope Confusion
**What goes wrong:** MCP server only works in one project or for one user
**Why it happens:** Wrong scope used during `claude mcp add`
**How to avoid:** Understand scopes:
  - `local` (default): One user, one project
  - `project`: Team via `.mcp.json` in repo
  - `user`: One user, all projects
**Warning signs:** "Server not found" when switching projects

## Code Examples

### MCP Server Configuration for Plugin Distribution
```json
// .mcp.json at plugin root
// Source: https://code.claude.com/docs/en/mcp
{
  "totui-mcp": {
    "command": "${CLAUDE_PLUGIN_ROOT}/target/release/totui-mcp",
    "args": []
  }
}
```

### Complete plugin.json with MCP Servers Inline
```json
// .claude-plugin/plugin.json
// Source: https://code.claude.com/docs/en/plugins-reference
{
  "name": "totui-mcp",
  "description": "MCP server for to-tui - terminal todo list manager",
  "version": "0.2.11",
  "author": {
    "name": "to-tui"
  },
  "repository": "https://github.com/grimurjonsson/to-tui",
  "license": "MIT",
  "keywords": ["todo", "task", "mcp", "productivity"],
  "mcpServers": {
    "totui-mcp": {
      "command": "${CLAUDE_PLUGIN_ROOT}/target/release/totui-mcp",
      "args": []
    }
  }
}
```

### Adding MCP Server via CLI (Alternative to Plugin)
```bash
# Source: https://code.claude.com/docs/en/mcp

# Add as user-scoped server (available in all projects)
claude mcp add --transport stdio --scope user totui-mcp -- /path/to/totui-mcp

# Add as project-scoped (creates .mcp.json)
claude mcp add --transport stdio --scope project totui-mcp -- /path/to/totui-mcp

# List configured servers
claude mcp list

# Check server status in Claude Code
/mcp
```

### Plugin Installation Commands
```bash
# Source: https://code.claude.com/docs/en/plugins

# Add marketplace from GitHub
/plugin marketplace add grimurjonsson/to-tui

# Install plugin from marketplace
/plugin install totui-mcp@grimurjonsson/to-tui

# Test plugin locally during development
claude --plugin-dir ./

# List installed plugins
/plugin list
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Claude Desktop config only | `claude mcp add` CLI + scopes | Claude Code 2.0+ | Simplified MCP setup |
| SSE transport | HTTP transport (recommended) | 2025+ | SSE deprecated for remote servers |
| Manual MCP config | Plugin marketplace system | 2025 | Easier distribution, versioning |
| Global-only MCP | Local/Project/User scopes | Claude Code 2.1+ | Better access control |

**Current best practices:**
- Use `${CLAUDE_PLUGIN_ROOT}` for all plugin-relative paths
- Prefer HTTP transport for remote MCP servers
- Use stdio transport for local binary servers
- Distribute plugins via GitHub for team sharing
- Provide pre-built binaries for Rust projects

## Open Questions

Things that need user decisions or further investigation:

1. **Binary distribution strategy**
   - What we know: Current `install-binary.sh` downloads from GitHub releases
   - What's unclear: Are binaries being uploaded to releases?
   - Recommendation: Verify GitHub Actions workflow creates release binaries

2. **Skill file organization**
   - What we know: `skills/todo-mcp/SKILL.md` exists with good documentation
   - What's unclear: Should skill be renamed to match plugin name pattern?
   - Recommendation: Keep as-is; skill name can differ from plugin name

3. **Plugin update workflow**
   - What we know: Users need to re-run install-binary.sh after updates
   - What's unclear: Can this be automated via hooks?
   - Recommendation: Document manual process; investigate Setup hook for automation

## Sources

### Primary (HIGH confidence)
- [Claude Code MCP Documentation](https://code.claude.com/docs/en/mcp) - Official MCP configuration guide
- [Plugins Reference](https://code.claude.com/docs/en/plugins-reference) - Complete plugin.json schema
- [Plugin Marketplaces](https://code.claude.com/docs/en/plugin-marketplaces) - Marketplace distribution guide

### Secondary (MEDIUM confidence)
- [Create Plugins](https://code.claude.com/docs/en/plugins) - Plugin creation tutorial
- [GitHub MCP Server Install Guide](https://github.com/github/github-mcp-server/blob/main/docs/installation-guides/install-claude.md) - Reference implementation

### Project-Specific (HIGH confidence - existing code)
- Current `.mcp.json` configuration (hardcoded path needs update)
- Current `.claude-plugin/plugin.json` and `marketplace.json`
- `scripts/install-binary.sh` for binary installation
- `skills/todo-mcp/SKILL.md` for skill documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Verified from official Claude Code documentation
- Architecture: HIGH - Based on official plugin reference and working examples
- Pitfalls: HIGH - Common issues documented in official troubleshooting

**Research date:** 2026-01-20
**Valid until:** 2026-02-20 (30 days - Claude Code ecosystem evolving but stable)

## Implementation Checklist

Based on this research, the implementation should:

1. [ ] Update `.mcp.json` to use `${CLAUDE_PLUGIN_ROOT}` instead of hardcoded path
2. [ ] Update `.claude-plugin/plugin.json` with repository, license, keywords
3. [ ] Verify `marketplace.json` version matches Cargo.toml
4. [ ] Ensure `scripts/install-binary.sh` works correctly
5. [ ] Update README with correct plugin installation instructions
6. [ ] Consider adding inline `mcpServers` to plugin.json as backup
7. [ ] Test plugin installation via `/plugin marketplace add`
8. [ ] Document user-scoped MCP addition as alternative to plugin
