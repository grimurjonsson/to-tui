# Kanban implementation

Objective: a project kanban plugin, available in the TUI, agent tools and a live web view, with comments, reasoned backward moves, durable agent rediscovery, and automatic safe migration of old databases.

Design: one persistent board per project, independent of daily todo rollover. Tickets have backlog, ready, in_progress, review, done and blocked states. Every change records actor, timestamp and history. Backward moves require a reason; that reason remains on the ticket until explicitly addressed. Ticket revision checks prevent agents from overwriting newer user feedback. Project UUIDs keep boards attached through project renames.

Remaining delivery gates:

- Storage migration and workflow tests, including old databases, concurrency, history, reason acknowledgement and project lifecycle.
- REST and remote client support; MCP and JSON CLI agent operations with discoverable guidance.
- TUI board creation, viewing, ticket editing/moving and comments.
- Live web board with comments, move reasons and reconnection behavior.
- Actual kanban plugin in sibling to-tui-plugins repository; connector extension if needed, rebuilding/upgrading both existing plugins.
- End-to-end verification, documentation, cargo fmt, clippy, build and full tests in both repositories.

## Current implementation checkpoint

`src/kanban.rs` provides shared board actions, serializable request/response types, automatic schema initialization, transaction-serialized writes, revision checks and ticket activity. `totui kanban --json` accepts a request (or `-` for stdin); MCP exposes `kanban`; REST exposes GET/POST `/api/kanban`; remote protocol forwards the same request to the selected workspace. Actor is currently a caller-supplied attribution label, not a verified identity.

Example requests:

```json
{"project":"default","actor":"user","action":"create_board","name":"Delivery"}
{"project":"default","actor":"agent","action":"create_ticket","title":"Implement search","description":"Search all board tickets","assignee":"agent"}
{"project":"default","actor":"agent","action":"view"}
{"project":"default","actor":"agent","action":"move_ticket","id":"TICKET_UUID","expected_revision":1,"status":"in_progress","reason":null}
{"project":"default","actor":"user","action":"comment","id":"TICKET_UUID","expected_revision":2,"body":"Include descriptions in search"}
```

Read operations return `null` when no board exists. Mutations return the entire board, including ticket revisions, comments and outstanding feedback. An agent must reload after a revision conflict. Comments are retained as activity; backward-move reasons additionally remain in `feedback` until `address_feedback` records a nonempty resolution.

The interface crate and sibling plugin repository have not yet changed. The next major work is a real loadable kanban plugin and TUI/web integration; current operations alone do not satisfy the full objective. Extend the connector deliberately, keeping existing plugins compatible or rebuilding them as required. Web live updates can reuse `/api/events` (database observation), but the board must be fetched/rendered by new UI code. HTTP workflow/error tests and actual multi-connection concurrency testing are also still needed.

## Web checkpoint (2026-09-09)

Implemented `/kanban`, linked from the existing tasks UI with the selected project. It supports creating boards and tickets, editing ticket details/assignees, selecting a destination column, adding move reasons and comments, recording feedback resolutions, and reading activity. `/api/events` drives live refresh with reconnect refresh and periodic recovery. Stale ticket dialogs preserve drafts and disable mutations until the latest revision is loaded. Comment/move/resolution actions preserve unsaved edits in other fields.

Verification: terminal-browser against an isolated workspace demonstrated browser board creation, automatic appearance of an external CLI-created ticket, browser comment persistence, stale dialog detection after a CLI move, browser reopening Done → Ready with a reason, and CLI rediscovery of that reason. A follow-up browser check confirmed adding a comment preserves an unsaved title draft. Screenshot inspection covered the narrow layout and ticket dialog. HTTP integration tests cover assets, board/ticket creation, change events, move reason validation, stale writes, comments, cross-origin protection, and remote protocol rediscovery. `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test`, and JS syntax checking passed.

Still required: TUI functionality, real loadable kanban plugin and connector integration/upgrades, multi-connection storage contention tests, broader UI/remote/auth/migration completion audit, final user documentation. Browser smoke server used only `/tmp/totui-kanban-web.hEfJI1` on port 48379; no real user todo data was changed. Browser tab 2 in terminal-browser 34936-1 is a test workspace.

## TUI checkpoint (2026-09-09)

F7 now opens the current project's board from the todo navigation screen. The help overlay lists it. The board uses adaptive horizontal columns, ticket selection, detail/activity scrolling and one-second refresh. Keys: arrows/hjkl to navigate, c to create the first board or comment on a selected ticket, n for a ticket, e to edit, 1–6 to move to a column, a to resolve feedback, r to refresh, Esc/q to return. Forms use Tab/Shift-Tab, Unicode-safe cursor editing, Enter for newlines and Ctrl+S to save. Existing todo navigation and editor bindings remain untouched. Forms capture ticket revisions and retain their draft on conflicts.

Tests cover the full TUI create/complete/reopen/comment/resolve workflow, stale form preservation, Unicode editing, and rendering at small/normal/wide terminal sizes. A real PTY session opened the empty board with F7, created a board and ticket through key input, and confirmed the agent CLI could read the same ticket and description. `cargo fmt`, strict all-target Clippy, build and the full test suite passed after these changes.

Next required work: actual kanban plugin in the sibling repository, connector support and rebuilding/upgrading existing plugins as needed; stronger contention/remote/auth tests and the final scope audit. All previous references to the TUI being absent are historical checkpoints.

## Connector/plugin checkpoint (2026-09-09)

Found authoritative mismatch: sibling plugin sources already depended on interface 0.4 interactive action/callback/on_event APIs, but main exposed 0.3. The existing `feat/jira-augment` branch contains that implementation. Merged only its `crates/` and `src/` changes relative to its merge base with main (not whole old files), resolving formatting conflicts and preserving current remote/web/TUI changes. This brings entry menus, picker/confirm callbacks, snapshot host API for hooks and interactive response dispatch into main. New connector version is 0.5.0 with `HostApi::kanban_request` (current enabled project only, actor attributed by host) and `FfiActionResponse::OpenKanban`.

Created sibling `kanban/` cdylib plugin with open/create-board and JSON-action entries. Updated jira-claude to 0.2.1 and claude-tasks to 1.0.6, both requiring interface 0.5.0; dependency paths now relative to sibling to-tui. Updated marketplace versions. All three plugins built in debug. Existing plugin tests pass (Jira 10, Claude tasks 109); kanban strict Clippy passes. Removed unused Jira transitions stub; test-only transition type remains, and actual transition names now appear in status messages. Host strict Clippy/build/full workspace tests pass after integrating connector changes. An explicit dynamic-library test loaded `kanban/target/debug/libkanban.dylib`, invoked create-board through FFI, verified persisted board and project access denial.

Outstanding: user-installed plugin directories are still unchanged (`~/Library/Application Support/to-tui/plugins/{jira-claude,claude-tasks}`). Need build/release/install matching host and plugins together so old installed host isn't stranded with incompatible libraries; back up existing artifacts. Audit branch-ported action flows, avoid regressions, verify actual plugin menu, old-plugin compatibility/error handling, remote/auth/concurrency and final docs. Audit plugin formatter changes for scope. Release packaging/justfile/CI may need kanban additions. Dynamic test invocation: `TOTUI_TEST_KANBAN_LIBRARY=/Users/gimmi/Documents/Sources/rust/to-tui-plugins/kanban/target/debug/libkanban.dylib cargo test --lib test_loadable_kanban_plugin_creates_board_through_host`. Normal suite skips that external-artifact test without the env var. Completion remains unproven.

## Installation/audit checkpoint (2026-09-09)

Installed matching release executables (`~/.local/bin/{totui,to-tui,totui-mcp}`) and plugin libraries/manifests/READMEs in `~/Library/Application Support/to-tui/plugins`. Backups of all old executables, both old plugin directories, and SQLite database are in `/Users/gimmi/.local/share/to-tui-backups/kanban-20260909T021257Z`. Plugin list confirms kanban 0.1.0, jira-claude 0.2.1, claude-tasks 1.0.6. An installed-binary PTY session selected P → kanban → Create project board, entered Menu board, and opened the board successfully in an isolated data root.

Fixed action dispatcher returning to Navigate after terminal Commands/Status responses, and queueing interactive hook responses until editors/modals close (instead of dropping them or overwriting the editor). Added tests. Added real SQLite contention test with 8 connections; exactly one revision-1 move succeeded and all others conflicted. All three release plugin libraries load against connector 0.5; kanban create-board FFI test passed. Strict Clippy is clean in host and all plugin crates (fixed pre-existing Jira file open options and Claude logging/test lint warnings). Updated README kanban usage and plugin release workflow for kanban plus connector checkout. Main workspace full tests pass (404 passed + 1 ignored lib, 67 each binary, 22 connector plus doctests).

Local server restart exposed a pre-existing child startup bug: default remote config causes a child lacking --local to exit. Fixed child --local. It then answered health in the spawning tool but died after the shell ended, so added a separate Unix process group for background launch. Release rebuild for that final change was started in exec session 2866; must poll it and install the newly built executables (keeping original backups) before verifying local server stays up across tool calls. Last server on port 48372 is currently unverified/down. Earlier installed server PID 37701 was explicitly stopped for the upgrade.

Completion still requires final server verification, remaining remote/auth/ABI regression audit and artifact checks. No publish/commit performed.

Follow-up: final daemon fix release build and strict Clippy passed. Reinstalled matching executables. Server startup completed in session 87059; a separate subsequent call returned HTTP 200 for `http://127.0.0.1:48372/kanban` and `serve status` confirmed PID 41481 running. This supersedes the down/unverified-server note above.

## Final audit

See `docs/kanban-verification.md` for the completed requirement-by-requirement audit. Final checks caught and fixed the MCP optional-root output schema startup panic and fresh-agent initialization. Real MCP stdio workflow and authenticated browser/remote isolation tests now pass. Latest release binaries are installed, hashes match, and local server assets/health return 200 across tool calls. This supersedes all earlier pending-work notes.
