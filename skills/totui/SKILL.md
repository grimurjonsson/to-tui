---
name: totui
description: Mirror in-progress work into the totui MCP todo list so the user can watch scope and progress live in their TUI. Use when the user says "totui", asks to track work in totui, or wants live visibility into what is being worked on and how much is left. Defaults to the "default" project.
---

<objective>
Give the user a live, readable view of work in flight. Before starting a multi-step task, write its scope into totui as a nested tree; as the work proceeds, keep exactly one leaf marked in-progress and flip finished leaves to done.

This is a **broadcast channel, not a scratchpad**. The internal todo list (TaskCreate/TaskUpdate) stays the working record; totui carries a stable, human-readable projection of it.
</objective>

<quick_start>
Default project is `default`. Only pass `project` when the user names a different one.

Three calls cover almost everything:

```
list_todos     { project }
create_todo    { content, description?, parent_id?, project }
update_todo    { id, state?, content?, description?, project }
```

Those are **bare** names. The callable name carries a prefix that depends on how
the server was installed — `mcp__plugin_<marketplace>_<server>__` when installed
as this plugin, `mcp__<server>__` when wired directly in `.mcp.json`. Match the
tool in your available tools whose name *ends with* the bare name; never hardcode
a prefix. If none is present, the to-tui MCP server is not connected — say so
rather than guessing.

Opening move for a new task:

1. `list_todos` — look for an existing root this work belongs under.
2. Create the tree: root (if none matched) → one child per milestone.
3. Set the first leaf to `*` and its ancestors to `*`.
4. Work. After each milestone: that leaf → `x`, next leaf → `*`.
5. At the end: all leaves `x`, ancestors `x`.
</quick_start>

<states>
The `state` field takes a single character. The `update_todo` **parameter** description omits `*` — that is a documentation bug in the server, not a missing feature. `*` works and reports back as `state_description: "in_progress"`.

| State | Meaning | Use for |
|---|---|---|
| `' '` | pending | Planned, not started |
| `*` | in progress | **The one leaf being worked on right now**, plus its ancestors |
| `x` | done | Finished and verified |
| `!` | important | A blocker, or something needing the user's attention |
| `?` | question | Blocked awaiting a user decision |
| `-` | cancelled | Dropped from scope — keeps the record without implying it was done |

`create_todo` has no `state` parameter — items are always born pending. To start something in progress, create it then `update_todo` it to `*`.

**Exactly one leaf carries `*` at a time.** That is the whole point: the user glances at the TUI and sees where you are. Two in-progress leaves means the signal is lost.

Ancestors of the active leaf also carry `*`, so a collapsed tree still shows which branch is live.
</states>

<granularity>
Write **milestones, not moves**. A good leaf is something that takes minutes and whose completion is a fact the user would care about. A bad leaf is a tool call.

```
[*] Fix flaky auth tests
    [x] Reproduce failure locally
    [x] Trace to token clock skew
    [*] Patch refresh window          <- exactly one active leaf
    [ ] Add regression test
    [ ] Run full suite
```

Not this:

```
[ ] Fix flaky auth tests
    [x] Read auth_test.go
    [x] Grep for TokenRefresh
    [x] Read token.go:88-140          <- tool calls, not milestones
```

Sizing:

- Fewer than 3 leaves — the task is too small to be worth tracking. Skip totui and just do it.
- More than ~10 leaves — group them one level deeper rather than flattening.
- Depth beyond 3 levels is almost never worth it; the TUI gets hard to scan.

Put the *why* and the identifiers in `description`, not in `content`. Content should read as a short phrase; description carries commit SHAs, file paths, ticket keys, sizes.
</granularity>

<rooting>
Reuse an existing root when the work plainly belongs to it; create one only when nothing fits.

1. `list_todos` and read the top-level items.
2. Match on the work's subject — a ticket key, a project, a feature name. When a root matches, nest the new milestones under it (or under the right child of it).
3. No match → create a new root whose content names the task in the user's own terms.
4. Ambiguous match → ask, using AskUserQuestion. Do not guess between two plausible roots; a tree grafted in the wrong place is worse than one extra root.
</rooting>

<ownership>
**Only touch items created in the current session.** Items from earlier sessions, or edited by hand in the TUI, are the user's — do not restate, reword, re-state, or tidy them.

Two exceptions, both requiring the user to ask first:

- They explicitly ask you to update or reconcile an existing tree.
- They point at a specific stale item and ask you to fix it.

Never delete an item you did not create in this session. `delete_todo` removes children too, so a mistaken call on a root destroys the user's tree. When something needs removing, prefer marking it `x` or asking.
</ownership>

<workflow>
<starting>
Before the first real edit of a multi-step task:

- `list_todos` → decide the root per the rooting rules above.
- Create root (if needed) and the milestone leaves, top to bottom in execution order.
- `update_todo` the first leaf and its ancestors to `*`.
- Tell the user in one line what tree you created, so they know what to look at.
</starting>

<during>
At each milestone boundary, two calls:

- finished leaf → `x`
- next leaf → `*`

Do this **when the milestone actually completes**, not when you start thinking about it. The user is reading this live; an item marked done before its tests pass is a lie in their TUI.

When new work is discovered mid-task, add it as a new leaf in the right position rather than silently widening an existing one. Discovered scope is exactly what the user wants to see.

When blocked awaiting a decision, set the leaf to `?` and ask. When something needs attention but is not blocking, `!`.
</during>

<finishing>
- Every leaf reaches a terminal state: `x` if done, or left pending with a description saying why it was not.
- Ancestors → `x` only when all their children are terminal. A parent marked done over a pending child misrepresents the state.
- Do not delete the tree. Completed trees are the user's record of what happened.
</finishing>
</workflow>

<verification_before_done>
Never mark a leaf `x` on the strength of having written the code. Mark it `x` when the thing it claims is true — tests ran and passed, the build is green, the file exists with the content described.

If a verification step is itself a leaf ("Run full suite"), it cannot be `x` until the suite has actually run and you have read the result. This mirrors the general rule that evidence precedes assertions, and it matters more here because the user is treating the TUI as ground truth without re-checking your work.
</verification_before_done>

<anti_patterns>
- **Mirroring every internal todo.** The internal list churns; this one should not. Project milestones only.
- **Multiple active leaves.** Destroys the "where are you now" signal.
- **Marking done optimistically.** Mark done only against evidence, per the verification rule above.
- **Rewriting the user's items** to match your phrasing.
- **`delete_todo` on anything you did not just create.** It cascades to children.
- **Tracking trivial work.** A two-step task tracked in totui is noise; the user asked for scope visibility, not a receipt.
- **Passing `project` on every call out of habit.** It defaults to `default`; pass it only when the user named another.
- **Leaving a tree all-pending after finishing.** Worse than not tracking, because it reads as work that never happened.
</anti_patterns>

<success_criteria>
- The user can open the TUI mid-task and see, without asking: what the whole task is, which step is running, what is done, what remains.
- Exactly one leaf is `*` while work is in flight.
- Every `x` corresponds to something actually verified.
- Nothing the user wrote by hand, or that predates this session, was altered.
- The finished tree reads as an accurate account of what happened.
</success_criteria>
