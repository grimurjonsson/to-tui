# Kanban completion verification

## Keyboard navigation correction (2026-09-09)

Reproduced two web navigation failures: repeated `j` presses stayed one row below the original focused row, and the editor selection and keyboard cursor highlighted different rows. Navigation now moves browser focus, cursor, and selection together, updates an already-open editor while preserving drafts, and suppresses mouse-hover row shading until the pointer moves. Regression tests cover repeated movement in both directions with the editor open and closed, one highlighted row, retained drafts, and hover shading.

All 43 browser tests passed, along with Rust formatting, strict all-target Clippy, build and tests. The Linux release passed the two navigation regressions and two clipboard checks. Deployed from `/home/caitlyn/repos/to-tui-clipboard-20260909`, preserving the clipboard feature and sticky-note styling. Backup: `/root/totui-backup.VDX3sLME`. Installed/built SHA-256: `d92841f4eb5a93a395bcede941ae00a24ed112c01e439708d8d0ac2b5bc8396b`. Service readiness and authenticated HTTPS project reads passed; no recent service errors were logged.

## Web clipboard deployment (2026-09-09)

Deployed Copy task buttons and `y` shortcuts for daily tasks and Kanban tickets to Caitlyn. Copies use `Task name - description`, omitting the separator when the description is empty. The deployed build preserves the current chalk dividers and sticky-note styling from the Kanban design worktree.

Combined source: `/home/caitlyn/repos/to-tui-clipboard-20260909`. Pre-upgrade binary, unit, and database/state backup: `/root/totui-backup.5Ng8FUkL`. Installed and built binary SHA-256: `9990091dd9fa9c945422903114112776d5bd12d78acdc4ea7a41dde6de412d84`.

Validation: all 41 local browser tests, Rust formatting, strict all-target Clippy, build and tests passed. The combined Linux build passed eight Kanban browser tests and two daily-task clipboard tests, web syntax/format checks, and warning-free debug/release builds. After installation, service/database readiness and authenticated public HTTPS account/project reads passed. An isolated instance of the installed binary served clipboard and UI assets matching the combined source byte-for-byte. No recent service errors were logged.

Verified 2026-09-09 against the current worktrees and installed release artifacts. Historical checkpoints in `kanban-implementation.md` record intermediate gaps; this audit supersedes their outstanding-work notes.

| Requested outcome | Implementation and evidence |
| --- | --- |
| Project kanban plugin | `to-tui-plugins/kanban` is a native cdylib with open/create-board and JSON action entries. Release library loaded through the ABI checker and created a persisted board via the host. Installed plugin-menu PTY check created and opened “Menu board” in an isolated workspace. |
| Agent task creation and state changes | Shared actions exposed through CLI, MCP, REST and remote protocol. Real MCP stdio handshake and create-board/create-ticket/done/reopen/view/resolve/done sequence passed in a fresh workspace. CLI smoke tests and remote integration tests exercise the same records. |
| TUI creation and viewing | F7 and plugin menu open the project board. Keyboard workflow tests cover create, edit, comment, move, resolve, Unicode editing, stale drafts and small/normal/wide rendering. Real PTY-created tickets were read back through the agent CLI. |
| Live web monitoring | `/kanban` uses `/api/events` with reconnect and recovery refresh. Browser check saw external CLI-created ticket automatically; HTTP integration checks database mutation events. User controls cover details, assignees, comments, moves/reasons, resolutions and history. |
| User comments and reasoned backward moves | Persistent activity plus outstanding feedback. Browser and TUI checks supplied comments and backward-move reasons; subsequent CLI/MCP/remote reads rediscovered them. Completion is rejected until feedback is explicitly resolved. |
| Concurrent agents cannot overwrite feedback | Required ticket revision for mutations, SQLite immediate transactions. Eight independent connections attempted one revision; one succeeded and seven conflicted. Comment invalidation and stale TUI/web drafts verified. |
| Old database compatibility and automatic startup migration | Kanban tables/triggers are added transactionally at startup. Legacy-table migration test preserves todo content/description/project; repeated startup retains boards. Fresh-agent workspace test creates the directory/schema/default project automatically. Existing migration/rollover/storage suites pass. |
| Project lifecycle and workspace isolation | Board attaches to project UUID; rename preserves it, deletion cleans tickets. Cross-project ticket mutation is denied. Authenticated Alice/Bob test verifies isolated boards and ticket IDs, browser-to-remote feedback, unauthenticated denial and account-change protection. |
| Connector and existing plugins upgraded | Interface 0.5 incorporates the existing 0.4 interactive plugin API and adds project-scoped kanban requests/OpenKanban. Jira 0.2.1 and Claude tasks 1.0.6 rebuilt and installed. All three release dylibs pass actual ABI loading. Existing plugin tests and strict lint pass. Interactive response mode/queue regressions covered by tests. |
| Installed result works | SHA-256 comparisons verify all 3 installed host executables, 3 plugin libraries and 3 plugin manifests match verified artifacts. Installed menu check passes. Upgraded local server remains running across tool calls, and `/kanban`, JS, CSS and health return 200. |

Final gates passed:

- Host: formatting, strict all-target Clippy, release build, `cargo test --workspace`, JS syntax, whitespace check.
- Workspace tests: 407 library tests passed (1 pre-existing ignored), 67 tests per TUI binary, 22 interface tests, supported doctests passed. Dynamic plugin test environment explicitly enabled for this run.
- Plugin repository: formatting for all three crates, strict all-target Clippy for all three, Jira's 10 tests, Claude tasks' 109 tests, release builds, whitespace check.
- Real browser, terminal, CLI, MCP stdio and authenticated remote HTTP checks described above used isolated temporary data for mutations.

Installed executables: `~/.local/bin/totui`, `to-tui`, `totui-mcp`. Installed plugins: `~/Library/Application Support/to-tui/plugins/`. Local board URL: `http://127.0.0.1:48372/kanban`.

Original executable/plugin backups and SQLite backup: `/Users/gimmi/.local/share/to-tui-backups/kanban-20260909T021257Z`. No commits, marketplace publication or release tags were made; source changes remain reviewable in both worktrees. The release workflow now includes kanban and checks out the host connector when publishing is requested in the future.


## Caitlyn deployment correction

The initial installation covered only the local machine. The configured TUI default is `home` at `https://totui.gimmi.is`; that rollout was incomplete until the server deployment below.

On 2026-09-09, connected using `ssh caitlyn@caitlyn`, built the current source with Rust 1.93.1 (`cargo build --locked --release --bin totui -j 2`) in `/home/caitlyn/repos/to-tui-kanban-20260909`, and installed it through the existing upgrade helper. The Linux release build completed without warnings. The package version remains 0.7.1; this is the current worktree build, not a new published release.

- Consistent pre-upgrade backup of the binary, service unit and entire state tree: `/root/totui-backup.W6zzNGIg` on Caitlyn.
- Installed binary SHA-256 matches the built artifact: `11e8e019dbbbd4df69e63737abfd80a2caf496510d6bb0c2847200aaaa346ee6`.
- `totui.service` is active and readiness succeeds; no recent service errors were logged. Existing authentication and systemd drop-ins remain in place.
- Live authenticated HTTPS test created a temporary project, board and ticket; added a comment; completed and reopened the ticket with a reason; rediscovered both comment and feedback using the installed `totui --remote home kanban` CLI; resolved feedback and completed the ticket. The temporary project was deleted and deletion verified.
- SQLite integrity checks passed for the shared database, account database and active user's workspace database. The active user's workspace contains both automatically migrated kanban tables. Unopened legacy workspaces migrate when initialized.
- Remote board URL is `https://totui.gimmi.is/kanban`. Unauthenticated asset requests correctly return 401 at the application. Browser-session rendering was tested locally earlier; this deployment check used native authenticated HTTPS because no saved browser cookie was available.

## Drag-and-drop follow-up (2026-09-09)

Web cards support native desktop dragging between columns with drop highlighting. Forward drops persist immediately using the revision captured at drag start. Backward drops open the existing editor with the destination selected and focus the required reason; cancellation leaves the ticket unchanged. Live refresh is deferred during dragging, and stale writes retain concurrent feedback and display the existing conflict controls.

`web/tests/kanban.spec.js` exercises real browser dragging and persistence, backward reason validation, and a concurrent CLI comment during drag. All three tests passed, including same-column drops and cancelling backward moves. Host formatting, strict all-target Clippy, debug build and workspace tests passed; JavaScript syntax and whitespace checks passed.

Deployment preflight confirmed the installed server still matches the prior audit's SHA-256 (`11e8e019...46ee6`). The deployment snapshot's Rust and existing web sources match the current workspace (ignoring macOS metadata files); only the drag-and-drop JS/CSS were transferred for this update.

Deployed the drag-and-drop assets to `home` using the existing upgrade helper after a warning-free Linux release build. Backup: `/root/totui-backup.MVhVqrai` on Caitlyn. Installed and built binary SHA-256 both equal `ff0003553766855801d210b7651216e0d776cc27ac7118e2b2337f3983cc8924`. Service is active and `/api/ready` succeeds. Both embedded assets match the tested source byte-for-byte; authenticated CLI rediscovery confirms the existing project board and all six tickets remain accessible.


## Compact board and recoverable Trash

Backlog is a full-width section below the five active stages. Web columns wrap at narrower widths without horizontal scrolling. Cards and the editor expose To backlog and Trash; To backlog focuses the required reason. Trash is a persisted, revision-checked flag with activity history, recoverable from the collapsed Trash section. Existing stored tickets default to not trashed without a schema rewrite. Agents receive the flag and new trash_ticket/restore_ticket actions. The TUI renders Backlog below the active grid and exposes Tab/b/x/t/u navigation and actions.

Validation: 409 library tests passed (one existing ignored), 67 tests per TUI binary, 22 interface tests, doctests, strict Clippy, formatting and debug/release builds. Five browser checks cover drag conflicts, required backward reasons, cancel/no-op moves, 1440/1024/768/390px widths with no horizontal overflow, full-width lower Backlog, and quick trash/restore preserving feedback and details. Desktop and mobile screenshots were visually inspected. TUI keyboard checks include backlog/trash/restore, and storage tests cover old serialized tickets and stale restore rejection.

Local executables installed with backup at `/Users/gimmi/.local/share/to-tui-backups/kanban-layout-20260909T094930Z`.

Deployed the compact board build to Caitlyn from `/home/caitlyn/repos/to-tui-kanban-layout-20260909`. Pre-upgrade binary/unit/state backup: `/root/totui-backup.08AomBON`. The installed and built binary SHA-256 both equal `c7c5518a462d929e0fc0a54c8c6c3716dd0594c41a1175b4953734f4aef26d1e`. Service readiness and active status passed with no recent error logs. Authenticated live HTTPS smoke verified comments, backward reasons, CLI feedback rediscovery, completion, trash and restore with status/history preserved. Its temporary project was removed and removal verified.

## Backlog rows and Completed archive

The subsequent layout correction replaces Backlog cards with one ticket per row, including To board (moves to Ready) and Trash controls. Active cards no longer have To backlog or Trash buttons. Done cards have Archive; it moves tickets into the Completed list immediately below Backlog. Completed defaults collapsed and exposes Restore to Done. Archiving is explicit, not automatic or timed. The archived flag defaults false for old records and preserves status, feedback and all activity. Archived tickets require unarchive before further mutations, and every archive/restore checks the ticket revision. CLI/MCP expose archive_ticket and unarchive_ticket; the TUI uses z to archive/restore and v to view Completed.

Validation passed: 410 library tests (one pre-existing ignored), 67 tests per TUI binary, 22 interface tests, doctests, formatting, strict Clippy and debug/release builds. Six browser checks cover drag/drop/conflicts, row placement and no horizontal overflow at 1440/1024/768/390px including long titles/assignees, backlog-only shortcuts, and the collapsed Completed archive/restore lifecycle. Desktop and mobile screenshots were inspected. Local final executables installed with pre-change backup `/Users/gimmi/.local/share/to-tui-backups/kanban-completed-20260909T100028Z`.

Deployed the final row/Completed build to Caitlyn from `/home/caitlyn/repos/to-tui-kanban-completed-20260909`. Fresh pre-upgrade backup: `/root/totui-backup.uLF0zbIM`. Installed/built SHA-256 match: `9dab291c39e6b8968d22a4f42fd9fcfc9a4598f5b1a83b0ddcbccba5359dfcdb`. Service active/readiness checks passed with no recent errors. Live authenticated HTTPS archive/unarchive preserved Done status and comment history; the existing feedback and trash/restore smoke also passed. Temporary verification project removed and deletion verified.
