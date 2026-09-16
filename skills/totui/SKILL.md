---
name: totui
description: Manage todos and mirror work into the user's todo list through the totui CLI and selected server API. Use for todo management, daily planning, and live progress tracking. Resolve the current folder's project and announce the destination before writing. No MCP connection required.
---

# Todo management and live progress tracking

Give the user a stable, readable view of work in flight. Track meaningful milestones, not individual tool calls. Keep your detailed working notes elsewhere.

## Choose the destination before writing

Use `totui todo` through the shell tool. **Do not use legacy MCP tools or access the database directly**, even if those tools remain connected. The CLI reuses the TUI's remote configuration, credentials, and folder-to-project mapping.

1. Set the shell tool's working directory to the user's active project folder.
2. Run `totui todo context`. For a user-specified project, use `totui todo context --project NAME`. Respect explicit backend preferences with `--remote NAME` or `--local`.
3. Read the returned JSON: `backend`, `remote`, `server_url`, `project`, and `folder`. Never assume `default`.
4. **Tell the user where the tree will go before creating it.** Example: “I'll track this under Codex in project `to-tui` on `home` (`https://totui.gimmi.is`).” For local storage, explicitly say “local project `NAME`.” Announce; don't ask for permission when tracking is already authorized.
5. Pin the resolved backend and project on all subsequent commands. Keep the same working directory. Re-resolve if the user changes the destination or workspace.

If the user expects remote but context reports local, do not write. Explain the mismatch. If `todo context` is unavailable, the installed CLI needs upgrading. Network or login failure must never cause fallback to local storage or MCP.

## Build the tree

For progress tracking, use one agent root named after the host: `Codex`, `Pi`, `Oh My Pi`, `Claude Code`, `OpenCode`, etc. Everything you create for tracking goes under that root. Existing top-level user items are not places to insert agent progress unless explicitly requested.

Assuming context returned remote `home`, project `to-tui`:

```sh
totui --remote home todo list --project to-tui
totui --remote home todo create --project to-tui --content Codex
totui --remote home todo create --project to-tui --parent-id ROOT_ID --content 'Implement live synchronization'
totui --remote home todo create --project to-tui --parent-id TASK_ID --content 'Verify reconnects and conflicts'
totui --remote home todo update LEAF_ID --project to-tui --state '*'
```

Substitute the actual resolved values. For local mode use `--local`. Read the JSON IDs from successful create responses. Reuse an existing agent root; create one only if absent. Put a new task tree under it, or extend a matching tree only when the user has authorized updating that existing tree.

Use 3–10 meaningful milestone leaves per task. Skip progress tracking for trivial work with fewer than three milestones. Put explanatory detail in `description`, not long titles. Use `--json -` with safely supplied JSON for multiline text.

After creating the tree, **report where it actually landed**, for example: “Created `Implement live synchronization` under Codex in remote project `to-tui` on `home`.” One destination announcement/report per tree is enough; individual leaf updates need not repeat it.

## Keep progress honest

Exactly one leaf should be `*` while work is in progress. At a milestone boundary, mark the verified leaf `x` and the next leaf `*`. Mark ancestors in progress only if you created them in this session; don't alter an existing user-owned root's state.

| State | Meaning |
|---|---|
| space | Planned |
| `*` | In progress |
| `x` | Finished and verified |
| `?` | Waiting on a user decision |
| `!` | Needs attention |
| `-` | Cancelled |

Never mark a milestone complete merely because code was written. Test or otherwise verify its claim first. Add a milestone when material new work appears. Don't leave active work looking pending.

A failed write stays failed: don't announce success, overwrite a conflict, or switch backends to make it succeed. Reload and handle concurrent changes deliberately.

## Ownership and finishing

Only modify items created in this session unless the user explicitly authorizes changing existing items. Never delete a user-owned item; deletion also removes children. Prefer cancellation for dropped work. Creating a child under an existing agent root does not authorize changing that root or its other children.

When finished, mark all verified leaves and newly created ancestors `x`. Keep the completed tree as a record. If work remains incomplete, leave its status accurate and explain why. In the final report, identify the project/backend where the progress was recorded.

## CLI reference

For ordinary todo management, follow the user's requested organization; an agent progress tree is not required.

```sh
totui --remote home todo get UUID --project to-tui
totui --remote home todo update UUID --project to-tui --state x
totui --remote home todo move UUID --project to-tui --parent PARENT_UUID
totui --remote home todo delete UUID --project to-tui
totui --remote home todo projects
```

Substitute the resolved backend/project. `projects` takes no project flag. `--date YYYY-MM-DD` selects a day's list; it defaults to today. Listing today may roll incomplete items forward if no list exists yet.

Commands print JSON on stdout; preserve it separately from destination messages on stderr. Lists return item arrays; get/create/update/move return an item with its ID; delete returns deleted IDs; projects returns names.

Create accepts `content`, `description`, `state`, `due_date`, `parent_id`, and `priority`. Update accepts `content`, `description`, `state`, `due_date`, `priority`, `clear_due_date`, `clear_priority`, and `expected_revision`. Omitted fields stay unchanged; an empty description clears it. Project and date are flags, not JSON fields. Priorities are `P0`, `P1`, `P2`, or unset.

Supply structured JSON through stdin with `--json -` or a safely written file. Never interpolate user content into shell code. Check `totui todo SUBCOMMAND --help` for flags. Never copy credentials into commands.

On conflict, reload and resolve deliberately; never silently retry a stale mutation with refreshed versions. Set state `x` explicitly to complete an item rather than toggling it. When displaying todos, name the project/backend and preserve hierarchy, states, priorities, and due dates.
