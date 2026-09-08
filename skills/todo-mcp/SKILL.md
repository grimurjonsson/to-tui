---
name: todo-mcp
description: Interact with the todo-mcp server to list, create, update, complete, and delete todos with hierarchical nesting support. Use when user asks about their todos, task management, daily planning, or mentions "my todos".
---

<objective>
Manage todos via the to-tui MCP server. Supports hierarchical todos (parent/child relationships), due dates, descriptions, priorities (P0/P1/P2), and six states including "in progress".
</objective>

<states_and_priorities>
Every item has a **state**, a single character:

| State | Meaning | Shown in `formatted` as |
|---|---|---|
| `' '` (space) | pending, not started | ⬜ (🔳 if it has completed children) |
| `'*'` | in progress | 🔄 |
| `'x'` | done | ✅ |
| `'?'` | question, blocked on the user | ❔ |
| `'!'` | important, needs attention | ❗ |
| `'-'` | cancelled, dropped from scope | 🚫 |

Every item may also carry a **priority**: `P0` (critical), `P1` (high), `P2` (medium), or none. It appears in `formatted` as a `[P0]`-style badge after the checkbox, and in `items` as the `priority` field (absent when unset).

**When you work on an item, mark it in progress; when you finish, check it off.**
Before starting a todo, `update_todo` it to `'*'`. When the work is done and verified, set it to `'x'` (or call `mark_complete`). Keep exactly one item in progress at a time so the user can see in their TUI what you are doing right now. Use `'?'` when you are blocked waiting on the user, `'!'` to flag something needing attention, and `'-'` instead of deleting when work is dropped.
</states_and_priorities>

<tool_names>
Tools below are named bare (`list_todos`, `create_todo`, ...). The **actual**
callable name carries a prefix chosen by the agent host and by how the server was
installed, so it is not stable across hosts or installs:

| Host / install path | Prefix | Example |
|---|---|---|
| Claude Code, installed as a plugin | `mcp__plugin_<marketplace>_<server>__` | `mcp__plugin_totui-mcp_totui-mcp__list_todos` |
| Claude Code, server in `.mcp.json` | `mcp__<server>__` | `mcp__totui-mcp__list_todos` |
| Codex CLI (`codex mcp add`) | `mcp__<server>__` (or `<server>__`) | `mcp__totui-mcp__list_todos` |
| OpenCode, pi, other MCP hosts | host-specific | ends with `list_todos` |

**Never hardcode a prefix.** Find the tool whose name *ends with* the bare name
in your available tools and call that. If no such tool is present, the to-tui MCP
server is not connected — say so rather than guessing at a name.
</tool_names>

<quick_start>
<list_todos>
**Tool**: `list_todos`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `date` | string | No | Date in YYYY-MM-DD format. Defaults to today. |
| `hide_completed` | boolean | No | When `true`, omit done (`x`) and cancelled (`-`) items. A completed parent stays if it still has unfinished children. Defaults to `false`. |

The response also carries `item_count` (items returned), `hidden_count` (items omitted by `hide_completed`), and `items` (raw data: `id`, `content`, `state`, `state_description`, `indent_level`, `parent_id`, `due_date`, `description`, `priority`).

Use `hide_completed: true` when the user asks what is left, or when a long list is mostly done. When items are hidden, the header says so (for example `(7/10, 7 completed hidden)`), so the user still sees the overall progress.

**CRITICAL**: The response contains a `formatted` field with pre-formatted markdown.

**MANDATORY**: Display the COMPLETE `formatted` field. NEVER truncate, summarize, or hide items.

Wrap in a code block to preserve formatting:

~~~
```
## Todos for 2026-01-02 (5/10)

🔄 [P0] Task being worked on right now
⬜ Task one (due: 2026-01-05)
✅ Completed task
  ⬜ [P2] Subtask
❔ Waiting on a decision
🚫 Dropped task
```
~~~

**FORBIDDEN**: 
- "(+ N more completed)" 
- "X completed, Y remaining"
- Hiding/collapsing completed items
- Any form of summarization

Automatically rolls over incomplete todos from previous days if today's list is empty.
</list_todos>

<create_todo>
**Tool**: `create_todo`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `content` | string | Yes | The todo text. Cannot be empty. |
| `description` | string | No | Additional notes or details. |
| `due_date` | string | No | Due date in YYYY-MM-DD format. |
| `parent_id` | string | No | UUID of parent todo to nest under. Get IDs from `list_todos`. |
| `priority` | string | No | `P0` (critical), `P1` (high), or `P2` (medium). Omit for none. |
| `date` | string | No | Which day's list to add to. Defaults to today. |

New items are always created pending. To start on one immediately, follow up with `update_todo` and `state: "*"`.

Example - create nested todo with due date and priority:
```
content: "Review PR #123"
description: "Check for security issues and test coverage"
due_date: "2026-01-05"
priority: "P1"
parent_id: "4497a476-61d0-4f13-9603-65b1eae5e37f"
```
</create_todo>

<update_todo>
**Tool**: `update_todo`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | UUID of the todo to update. |
| `content` | string | No | New content text. |
| `description` | string | No | New description. Empty string clears it. |
| `due_date` | string | No | New due date in YYYY-MM-DD format. |
| `state` | string | No | New state: `' '`, `'*'`, `'x'`, `'?'`, `'!'`, or `'-'` (see the states table above). |
| `priority` | string | No | `P0`, `P1`, or `P2`. |
| `date` | string | No | Date in YYYY-MM-DD format. Defaults to today. |

Typical lifecycle of an item you are working on:

```
update_todo { id, state: "*" }    <- starting
... do the work ...
update_todo { id, state: "x" }    <- finished and verified
```
</update_todo>

<mark_complete>
**Tool**: `mark_complete`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | UUID of the todo to toggle. |
| `date` | string | No | Date in YYYY-MM-DD format. Defaults to today. |

Toggles completion: marks pending as done `[x]`, or done as pending `[ ]`.
</mark_complete>

<delete_todo>
**Tool**: `delete_todo`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | Yes | UUID of the todo to delete. |
| `date` | string | No | Date in YYYY-MM-DD format. Defaults to today. |

**Warning**: Deletes the todo AND all its children. Irreversible.
</delete_todo>
</quick_start>

<workflow>
**Common patterns**:

1. **Show todos**: Call `list_todos`, wrap `response.formatted` in a code block to preserve checkbox formatting
2. **Show what's left**: Call `list_todos` with `hide_completed: true`
3. **Add subtask**: First `list_todos` to get parent UUID, then `create_todo` with `parent_id`
4. **Work on a task**: `update_todo` to `'*'` before starting, `'x'` when done
5. **Complete task**: Use `mark_complete` with the todo's `id`, or `update_todo` with `state: "x"`
6. **Prioritize**: `update_todo` with `priority: "P0"` / `"P1"` / `"P2"`
7. **Bulk operations**: Chain multiple tool calls for efficiency
</workflow>

<anti_patterns>
**DO NOT**:
- Reformat the `formatted` field output (it's already properly formatted)
- Strip the `[ ]` or `[x]` checkbox markers
- Change indentation of nested items
- Convert to a different list format
- Display as raw markdown (brackets get stripped) - always use code block
- Truncate or summarize the list (e.g., "+ N more completed")
- Hide completed items yourself - if the user wants only open items, ask the server with `hide_completed: true`
- Add summaries like "X completed, Y remaining"
- Leave an item you are actively working on in pending state - set it to `'*'`
- Mark an item `'x'` before the work is actually done and verified

**ALWAYS show the COMPLETE list exactly as returned. No exceptions.**
</anti_patterns>

<success_criteria>
- Todo operations complete without errors
- User sees formatted todo list when requested
- Hierarchical structure preserved (parent/child relationships)
- Due dates, priorities and descriptions displayed when present
- Items you work on go `' '` → `'*'` → `'x'`, with exactly one in progress at a time
</success_criteria>
