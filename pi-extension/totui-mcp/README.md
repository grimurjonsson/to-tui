# totui pi extension

Remote-aware todo tools, a panel, and a focused-task widget for pi. The directory
name `totui-mcp` is retained so existing installation symlinks keep working;
**the extension no longer uses MCP or a separately started local REST server**.

## Destination and transport

All tools and UI actions use `totui todo`. The CLI owns authentication and calls
the selected server API. On first use, the extension runs `totui todo context`
from **the session's cwd**, resolving the same folder/project and backend as the
TUI. Subsequent commands pin `--remote NAME` (or `--local`) and `--project NAME`.
No project is hardcoded to `default`.

The widget/status and tool responses identify the destination. Agent writes also
announce it before execution. `totui_context` lets the agent inspect/announce the
destination first. Remote failures surface as errors: no local/MCP fallback,
offline write queue, or automatic mutation retries. If a write times out, check
its outcome before retrying—it may already have reached the server.

An explicit tool `project` override is validated on the pinned backend without
changing the panel's project. `/totui-reconnect` re-resolves configuration; use it
after changing remote configuration or folder mappings. Backend pinning is by
remote **name**, so don't repoint that remote profile during an operation.

## Install

Requires a `totui` CLI supporting `todo context` (verified with 0.7.1).
Configure and authenticate a remote with `totui remote` first, if desired.

```sh
totui todo context
just setup-pi-extension
```

Then `/reload` in pi (or restart). Existing installed directory symlinks need no
change. There is no need to run `totui serve start` or install `totui-mcp`.

## Configure

```sh
pi --totui-command /path/to/totui  # default: totui on PATH
pi --totui-remote home            # otherwise use CLI configuration
pi --totui-local                  # explicitly select local storage instead
pi --totui-project MergeQuest     # otherwise use folder mapping
pi --totui-poll-ms 5000
pi --no-totui-widget
```

Environment equivalents: `TOTUI_COMMAND`, `TOTUI_REMOTE`, `TOTUI_PROJECT`,
`TOTUI_POLL_MS`. `--totui-local` and a remote override are mutually exclusive.
Flags take precedence over environment values.

**Migration:** `--totui-api-command` / `TOTUI_API_COMMAND` remain compatibility
aliases for the CLI binary; the modern `--totui-command` / `TOTUI_COMMAND`
settings take precedence. A retained legacy `totui-mcp-command=totui-mcp` default
is ignored (the MCP binary is never launched), allowing existing sessions to reload.
Remove `TOTUI_API_URL`, custom `TOTUI_MCP_COMMAND`, `TOTUI_MCP_ARGS` and their
`--totui-*` flag equivalents. These transport overrides produce an actionable
error rather than silently selecting a different backend. Use a configured CLI
remote, not a raw server URL. `--totui-auto-api` and `--totui-widget-roots` are
accepted but unused.

## UI (no LLM)

- `/totui`: toggle the scrollable panel.
- `/totui-refresh`: refresh the widget/status from the CLI.
- `/totui-unfocus`: clear all `[*]` focus in the selected workspace.
- `/totui-reconnect`: re-resolve backend/project and refresh.
- `⌘⇧T` or `Ctrl+Shift+T`: toggle panel.
- **Space/Enter** toggle done · **f** solo focus · **F** pin/unpin · **e/←/→**
  fold/unfold · **u** clear focus · **r** refresh · **Esc** close.

The compact widget shows `[*]` items. Collapsed branches persist across panel
close/reopen. Reads are coalesced while a poll is in flight; shutdown stops
polling, aborts CLI work, and suppresses late widget updates. Panel errors remain
visible even when there are no items.

In Cursor, Cmd+Shift+T is usually consumed by “Reopen Closed Editor”; use
Ctrl+Shift+T or `/totui`. Panel interaction requires TUI mode; tools also work in
print/JSON/RPC modes.

## Tools

Existing tool names are preserved, now backed by CLI commands:

| Tool | CLI operation |
|---|---|
| `totui_context` | `todo context` |
| `totui_list_todos` | `todo list` |
| `totui_create_todo` | `todo create --json ...` |
| `totui_update_todo` | `todo update ID --json ...` |
| `totui_delete_todo` | `todo delete ID` (including children) |
| `totui_mark_complete` | `todo get ID`, then `todo update ID --json ...` |
| `totui_list_projects` | `todo projects` |

Responses are JSON `{destination, date, result}` (`context` returns only
`destination`). Lists are bare item arrays under `result`; project lists are
**names**, not fabricated IDs/timestamps. Outputs above 50KB/2000 lines are
truncated with a path to the complete result. Tool definitions describe the new
shape/transport after `/reload`.

Dates default to the client's local today. Toggle pins the same date across its
read/write. Toggle and multi-item focus are **not atomic**; concurrent external
edits can race, and partial focus writes can succeed before a later failure.
Remote writes may reject historical dates. No credentials are read or copied by
the extension; CLI subprocesses use argv, not interpolated shell commands.

## Development

```sh
cd pi-extension/totui-mcp
npm install
npm test
npm run check
```

Tests use an injected runner and real subprocess fixtures, not your live todo
storage. They exercise registered tools, shared panel transport, folder/remote
selection, payload preservation, failures, cancellation, polling lifecycle and
error rendering.
