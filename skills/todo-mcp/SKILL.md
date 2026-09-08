---
name: todo-mcp
description: Manage todos through the totui JSON CLI and its selected server API. Resolve the current folder's project and announce the destination before writing. Use for todo management and daily planning; no MCP connection required.
---

# Todo management through the CLI

Use `totui todo` through the shell tool. The CLI manages authentication and calls the configured server API when a remote is selected. Never use the legacy MCP tools, read/write SQLite directly, or copy tokens into shell commands. The skill name is retained for compatibility; its transport is the CLI/API.

## Resolve and announce the destination

1. Run `totui todo context` with the shell tool's working directory set to the user's active project folder, not the agent host's launch directory. It returns `backend`, `remote`, `server_url`, `project`, `folder`, and `directory` as JSON.
2. If the user explicitly names a project, run `totui todo context --project NAME` to validate that destination. Respect explicit `--remote NAME` or `--local` preferences.
3. **Before creating any todos, tell the user the actual destination.** Example: “I'll add these under Codex in project `to-tui` on `home` (`https://totui.gimmi.is`).” For local mode say “local project `NAME`.” This is an announcement, not a permission question.
4. Pin that destination for every subsequent command: `totui --remote NAME todo ... --project PROJECT`, or `totui --local todo ... --project PROJECT`. Keep the same working directory. Re-resolve and announce if the user changes the workspace or destination.
5. After creation, tell the user which project/backend received the todos, using successful command output as evidence. For a task tree, one announcement before creating the tree and one report afterward suffice; don't repeat it for every leaf.

Never assume the project is `default`. It follows the same folder mapping and fallback as the TUI unless explicitly overridden. If context reports local mode while the user expects remote, stop writes and explain the mismatch. If `todo context` is unavailable, report that the client needs upgrading. If the server fails or authentication expires, report the error; do not retry using local storage or MCP.

## Commands

Commands print JSON on stdout. Mutations also identify the destination on stderr. Preserve JSON stdout separately from stderr when parsing it.

Examples below assume context returned remote `home`, project `to-tui`; substitute the actual resolved values.

```sh
totui --remote home todo list --project to-tui
totui --remote home todo get UUID --project to-tui
totui --remote home todo create --project to-tui --content 'Review API behavior'
totui --remote home todo create --project to-tui --parent-id PARENT_UUID --content 'Verify reconnects'
totui --remote home todo update UUID --project to-tui --state '*'
totui --remote home todo update UUID --project to-tui --state x
totui --remote home todo move UUID --project to-tui --parent PARENT_UUID
totui --remote home todo delete UUID --project to-tui
totui --remote home todo projects
```

`list` returns an array; `get`, `create`, `update`, and `move` return an item with its ID. `delete` returns the deleted IDs. `projects` returns project names and takes no `--project` flag. `--date YYYY-MM-DD` selects a day's list; it defaults to today. Listing today's todos may roll incomplete items forward if today has no list yet.

For multiline content, use a structured payload through stdin:

```sh
totui --remote home todo create --project to-tui --json - <<'JSON'
{"content":"Review API behavior","description":"Check reconnection and conflict handling.","priority":"P1"}
JSON
```

Use a quoted heredoc or a safely written JSON file; never interpolate user content into shell code. Create accepts `content`, `description`, `state`, `due_date`, `parent_id`, and `priority`. Update accepts `content`, `description`, `state`, `due_date`, `priority`, `clear_due_date`, `clear_priority`, and `expected_revision`. Omitted fields stay unchanged; an empty description clears it. Project and date are CLI flags, not JSON fields. Check `totui todo SUBCOMMAND --help` for other flags.

A conflict is an error, not permission to overwrite. Reload, explain the concurrent change, and ask how to resolve when necessary. Never silently retry a stale mutation with refreshed versions.

## States and presentation

| State | Meaning |
|---|---|
| space | Pending |
| `*` | In progress |
| `x` | Done |
| `?` | Needs clarification |
| `!` | Important |
| `-` | Cancelled |

Priorities are `P0` (critical), `P1` (high), and `P2` (medium), or unset. Mark work in progress when starting and done only when verified. Set state `x` explicitly to complete; don't toggle a task that might already be complete.

When showing a todo list, name its project and backend, preserve hierarchy, and render states, priorities, and due dates clearly. Honor the user's requested filtering. The CLI returns raw items, not an MCP `formatted` field.

Do not delete or rewrite the user's existing todos without authorization. For agent progress trees, follow the `totui` skill's ownership rules.
