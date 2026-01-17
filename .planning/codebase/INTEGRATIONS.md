# External Integrations

**Analysis Date:** 2026-01-17

## APIs & External Services

**Model Context Protocol (MCP):**
- MCP Server - Exposes todo management tools to LLM clients
  - Binary: `totui-mcp`
  - Transport: stdio (standard input/output)
  - Client: `rmcp` crate
  - Tools exposed:
    - `list_todos` - List todos for a date
    - `create_todo` - Create new todo item
    - `update_todo` - Modify todo content/state/due_date
    - `delete_todo` - Remove todo and children
    - `mark_complete` - Toggle completion state

**REST API:**
- Internal HTTP API for external integrations
  - Framework: Axum
  - Default port: 48372
  - Endpoints:
    - `GET /api/health` - Health check
    - `GET /api/todos` - List todos (query: date)
    - `POST /api/todos` - Create todo
    - `PATCH /api/todos/{id}` - Update todo
    - `DELETE /api/todos/{id}` - Delete todo
  - CORS: Enabled (allow all origins)
  - Tracing: HTTP request logging via tower-http

**Jira Integration (Plugin):**
- Generator plugin for creating todos from Jira tickets
  - External dependencies required:
    - `acli` - Atlassian CLI for fetching Jira tickets
    - `claude` - Claude CLI for generating todo breakdown
  - Implementation: `src/plugin/generators/jira_claude.rs`
  - Invocation: `totui generate jira <TICKET-ID>`

## Data Storage

**Databases:**
- SQLite (bundled via rusqlite)
  - Location: `~/.to-tui/todos.db`
  - Client: rusqlite with bundled SQLite
  - Tables:
    - `todos` - Active todo items indexed by date
    - `archived_todos` - Historical items after rollover
  - Features: Soft deletes, parent-child relationships, RFC3339 timestamps

**File Storage:**
- Local filesystem only
  - Daily markdown files: `~/.to-tui/dailies/YYYY-MM-DD.md`
  - Dual storage: Both SQLite and markdown (markdown as human-readable backup)

**Caching:**
- None - No external caching layer

## Authentication & Identity

**Auth Provider:**
- None required for local operation
- MCP: No authentication (relies on MCP client trust model)
- REST API: No authentication (localhost-only intended use)

## Monitoring & Observability

**Error Tracking:**
- None - Errors logged to stderr

**Logs:**
- Structured logging via `tracing` crate
- Output: stderr (for MCP server compatibility)
- Control: `RUST_LOG` environment variable
- Levels: error, warn, info, debug, trace
- HTTP tracing: tower-http TraceLayer

## CI/CD & Deployment

**Hosting:**
- Local installation only (no cloud hosting)
- Binaries distributed via GitHub Releases

**CI Pipeline:**
- GitHub Actions
- Workflow: `.github/workflows/release.yml`
- Trigger: Push of version tags (v*)
- Build matrix:
  - Linux x86_64 and aarch64 (using cross)
  - macOS x86_64 and aarch64 (native cargo)
  - Windows x86_64 (using cross)

## Claude Code Plugin

**Plugin Distribution:**
- Claude Code marketplace integration
- Config: `.claude-plugin/marketplace.json`, `.claude-plugin/plugin.json`
- Category: productivity
- Installation: Via GitHub URL or local symlink

## Environment Configuration

**Required env vars:**
- None required for basic operation

**Optional env vars:**
- `RUST_LOG` - Log level control (e.g., `debug`, `info,tower_http=debug`)

**Secrets location:**
- Not applicable - No secrets required
- Jira integration uses system-installed `acli` with its own auth

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

## External CLI Dependencies

**For Jira Plugin:**
- `acli` - Atlassian CLI (must be installed and authenticated)
- `claude` - Claude CLI (must be installed)

**Availability Check:**
- Plugin validates dependencies via `check_command_exists()` before use
- `src/plugin/subprocess.rs` handles command execution

## File Watching

**Database Watch:**
- Uses `notify` crate (8.2.0)
- Watches: `~/.to-tui/todos.db`
- Purpose: Auto-reload TUI when external changes detected (e.g., from API or MCP)
- Implementation: `src/ui/mod.rs` - `setup_database_watcher()`

## MCP Client Configuration

**OpenCode:**
- Config location: `~/.config/opencode/opencode.json`
- Setup command: `just configure-mcp-opencode`

**Claude Code:**
- Plugin format in `.claude-plugin/`
- Setup command: `just setup-mcp-claude-dev`
- Skill installation: `just install-claude-skill`

---

*Integration audit: 2026-01-17*
