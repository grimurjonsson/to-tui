# totui-mcp pi extension

Bridges [totui](https://github.com/grimurjonsson/to-tui) into [pi](https://github.com/earendil-works/pi-mono).

## Fast path (no LLM)

- **Compact widget** above the editor — shows **focused** todo only (`[*]` in-progress state)
- **`/totui`** — bordered scrollable panel (`⌘⇧T`, or `Ctrl+Shift+T` in Cursor terminal)
- **`/totui-refresh`** — manual refresh
- **Space** toggles done · **f** solo focus · **F** pin/unpin · **e** fold/unfold · **u** clear focus

**Focus model:** totui's `[*]` (in progress) state = focused. **f** clears other focuses and sets one; **F** toggles pin without clearing others. Widget/status bar show pinned items; no pins → header only.

**Collapse:** **e** (or ←/→) on a parent toggles ▾/▸; collapsed state is remembered until you expand again (persists across panel close/reopen).

**API auto-start (default):** on pi session start, runs `totui serve start --port <port>` if the API is not already up. The REST API is the only data source — no markdown file fallback.

## MCP tools (for agent mutations)

| Pi tool | MCP tool |
|---------|----------|
| `totui_list_todos` | `list_todos` |
| `totui_create_todo` | `create_todo` |
| `totui_update_todo` | `update_todo` |
| `totui_delete_todo` | `delete_todo` |
| `totui_mark_complete` | `mark_complete` |
| `totui_list_projects` | `list_projects` |

Guidelines steer the agent away from `list_todos` for viewing — use the panel instead.

## Install

```bash
just setup-pi-extension
```

Then restart pi or `/reload`.

## Configure

```bash
pi --totui-api-url http://127.0.0.1:3000
pi --totui-api-command totui       # binary for serve start
pi --totui-project default
pi --totui-poll-ms 5000
pi --totui-widget-roots 4
pi --no-totui-widget               # hide compact widget
pi --no-totui-auto-api             # don't auto-start API

export TOTUI_API_URL=http://127.0.0.1:3000
export TOTUI_API_COMMAND=totui
export TOTUI_PROJECT=default
```

MCP binary (for agent tools only):

```bash
pi --totui-mcp-command totui-mcp
```

## Commands

| Command | Action |
|---------|--------|
| `/totui` | Toggle todo panel |
| `/totui-refresh` | Refresh widget |
| `/totui-unfocus` | Clear all `[*]` focus |
| `/totui-reconnect` | Reconnect MCP client |
| `⌘⇧T` | Open panel (Ghostty/iTerm with super passthrough) |
| `Ctrl+Shift+T` | Open panel (Cursor integrated terminal fallback) |

**Cursor note:** `Cmd+Shift+T` is usually bound to “Reopen Closed Editor” and never reaches pi. Use `Ctrl+Shift+T` or `/totui` instead, or unbind the Cursor keybinding.
