# Local web workspace

The installed CLI works from any directory as either `to-tui` or `totui`:

```sh
to-tui web --detach --open
to-tui web --detach --restart
to-tui web start --port 48379
to-tui web restart --port 48379
to-tui web status
to-tui web --log
totui web logs --follow | lux
to-tui web stop
```

`web start` and `web restart` run in the background. Plain `web` stays in the
foreground; `web --restart` replaces the managed instance and stays in the
foreground unless `--detach` is also supplied. `--open`, `--verbose`, and `--port`
work before or after start/restart. Restart uses the supplied options (or defaults).
`--log` and `web logs` print the current log and exit. `web logs --follow` (or
`-f`) prints existing content, streams new output, and follows server restarts
until Ctrl+C. It waits if no log exists yet. Only log content goes to stdout,
so you can pipe it into `lux`; status messages go to stderr. Detached starts replace
`~/.to-tui/web/server.log` and wait for successful startup. Process state lives
alongside the log; `TOTUI_DATA_DIR` overrides this data root. Management supports
macOS/Linux and checks process identity before stopping a recorded process.
These commands manage their detached instance and recognize the older `serve`
daemon after checking its process identity and command. The checkout-specific
development wrapper remains separate. Foreground servers use Ctrl+C.

Install both executable names with `cargo install --path . --locked` from the
checkout once; subsequent commands need neither the checkout nor Cargo/Python.
Run `python3 scripts/test_web_cli.py` after building to verify the lifecycle with
isolated data from outside the checkout.

In the TUI, press **w** or click the **w web-ui** control in the footer to
open the web server panel. Use **↑/↓** to select **Start**, **Stop**, **Restart**,
or **Open in browser**, **Enter** to run the selected action, and **Esc** to close
the panel. The indicator stays
visible while editing and status refreshes every two seconds, including changes
made through the CLI. Operations run in the background without blocking the TUI.
The panel reuses the running server's port for subsequent starts/restarts.
The shortcut is configurable as `open_web_manager` in the navigation keybindings.

In Ghostty and Kitty, the footer repository link uses the bundled
`assets/github.png` after the terminal acknowledges the Kitty graphics protocol. It occupies the same two text cells as
the `🔗` fallback and keeps the same clickable area. The capability check is
bounded to 250 ms; other or unresponsive terminals retain `🔗`. Windows,
tmux, and screen currently use the fallback. The image uses Kitty Unicode
placeholders so it follows footer movement and overlays; resizing retransmits
it, and exiting deletes only this application's image.


The TUI retains automatic startup, now through `web start` when no server is
running. Stopping it keeps it stopped for the rest of the TUI session; reopening
the TUI starts it again. Closing the TUI leaves the server running. Existing API
daemons can be stopped or restarted in the panel; restart enables managed logs.
Untracked foreground servers are identified as external and must be stopped in
their own terminal. `python3 scripts/test_tui_web.py` exercises the panel in a PTY.

Run `totui web --open` and use the printed local address (by default
<http://127.0.0.1:48372>). `totui web --port 3000` chooses another port;
Ctrl+C stops the foreground server. `totui serve start` serves the same workspace,
API, and SSE endpoint and retains its existing daemon-management commands.
The HTML, CSS, and JavaScript are compiled into the Rust binary. Users do not need
Node, npm, or a frontend server.

Both server entry points now bind to **127.0.0.1**, replacing the previous
`0.0.0.0` default. `TOTUI_BIND` explicitly overrides the interface, including
`::1` for IPv6 loopback. `--open` uses the bound address, substituting the
corresponding loopback address when binding to all interfaces.

At startup, the web server picks the same project as the TUI: the current folder's
mapped project, then the last-used project, then `default`. The plain root URL
uses this startup selection. The printed URL and `--open` include `?project=NAME`
to explicitly select it. Switching projects updates this query parameter, so
refreshing or sharing a URL retains that project. A URL selection takes precedence
over the server's startup project.

## Troubleshooting a 404

Open <http://127.0.0.1:48372/> (or the port printed by `totui web`). The local
release binds to loopback; your machine's LAN address is a different destination.
An older API-only daemon can still answer on that address and return 404 for `/`
even while the new workspace works on loopback. A successful `/api/health` response
alone does not mean that the running executable includes the website.

When developing from this checkout, run `cargo run --bin totui -- web --open`;
`cargo build` does not replace an installed `totui` on your PATH. Stop an old managed
daemon with `totui serve stop` before restarting it from the updated executable.
If the foreground web server is already running, use its loopback URL rather than
starting a second server. Assets are bundled at compile time, so rebuild and
restart after frontend changes.

If browser changes do not appear in the TUI, check the TUI executable too:
`just dev` (or `cargo run --bin totui`) runs this checkout's current build, while
plain `totui` may still run an older installed version without live refresh.
Restart the TUI using the current build, select the same project/date, and use the
same `TOTUI_DATA_DIR` if set. Refresh waits while the TUI is editing or has unsaved
changes, then resumes in Navigate mode once those changes are saved.

The workspace supports existing project selection, nested tasks, all six states,
priority, due dates, descriptions, explicit descendant deletion, reparenting,
and insertion before a sibling. Branch controls and every editor action are
available as buttons or native form controls. On phones, the project picker
replaces the sidebar and the task editor becomes a full-screen dialog with
contained keyboard focus. Escape closes it. Drafts remain in memory when closing
an editor or browsing another project/date; reopening the same task restores the
draft. Reloading/closing the page prompts if unsaved drafts remain. Drafts are not
an offline-editing store.

Hide completed hides both done and cancelled tasks, retaining completed ancestors
of open descendants. Branch collapse is a browser-local preference for the page's
lifetime. Light/dark theme preference is stored in that browser.

## Isolated data and development

All interfaces use `~/.to-tui/todos.db` and project markdown exports under
`~/.to-tui/projects/<project>/dailies/`. Set **the same** `TOTUI_DATA_DIR` for the
web server, CLI, TUI, and MCP process to use another database. This changes the
application's data/config/log root, without changing your home directory. Plugin
installation directories retain their existing platform-specific locations.

```sh
cargo build
sandbox_data=$(mktemp -d)
TOTUI_DATA_DIR="$sandbox_data" cargo run --bin totui -- web --port 48379
# In another terminal, use the printed sandbox_data path:
TOTUI_DATA_DIR=/path/from/mktemp target/debug/totui
TOTUI_DATA_DIR=/path/from/mktemp target/debug/totui-mcp
```

Frontend sources are `web/index.html`, `web/style.css`, and `web/app.js`.
They use browser-native JavaScript, native form controls, Fetch, and EventSource;
there is no frontend bundler. Edit the sources, restart `cargo run --bin totui --
web`, and refresh the page to load newly bundled assets. CSS tokens at the top of
`style.css` define both themes. Runtime URLs are relative, so a reverse proxy can
mount the entire router under a path prefix ending in `/`.

Node/npm and Python 3 are development-only verification dependencies:

```sh
npm --prefix web ci
cd web && npx playwright install chromium && cd ..
npm --prefix web run format
npm --prefix web run check
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build
cargo test
npm --prefix web test
```

The Playwright suite launches the built Rust binaries with fresh temporary data
and ephemeral loopback ports. It tests real HTTP/SSE, separate CLI and MCP
processes, two browser tabs, desktop and touch-enabled phone layouts, draft/focus
preservation (including reordering above the viewport), conflicts, every state, optional-field clearing, branch controls,
deletions, reconnects, delayed fetches, project/date isolation, and nested
filtering. A POSIX PTY driver exercises actual TUI keystrokes and verifies that
browser changes reach its terminal output (this test skips Windows). Tests remove
their temporary data and stop their server. Screenshots and failure traces go in
`web/test-results/` and are ignored by Git.

`cargo test api::web::tests` additionally runs an isolated child test process for
external-process SSE notifications, a subscription with an arbitrary old event
ID, stale whole-list saves including empty saves, timestamp transitions, archive
browsing, rollover beyond 30 days, concurrent rollover, multi-list rollback, and
concurrent atomic snapshots. Existing undo/soft-delete tests exercise the new
transactional save path.

## Synchronization and concurrency

One dedicated SQLite connection per server observes `PRAGMA data_version` every
100 ms. It never writes. A Tokio watch channel shares invalidations with every
SSE connection; there is no database-polling loop per browser. Any committed
change on another connection—including a different CLI, TUI, MCP, or browser
process—is detected. The observer also compares the server's local date every
tick. Notifications are global invalidations, so project changes and archive or
ordering changes are covered without maintaining a separate event history.
See [SQLite's data_version contract](https://www.sqlite.org/pragma.html#pragma_data_version).

Every SSE subscription first emits `change`, regardless of Last-Event-ID. A
reconnect therefore always fetches a complete authoritative snapshot, including
deletions; event IDs are diagnostic sequence numbers, not durable replay offsets.
The browser subscribes immediately, coalesces notifications for 25 ms, and guards
fetch results with a request generation and project/date scope. It refreshes on
open/reopen, foreground, focus, pageshow, and network restoration, with an
additional 15-second foreground resynchronization as a safety net. Failed reads
remain visible as errors. SSE uses ten-second comment keepalives and native
EventSource automatic reconnect. See [MDN's SSE documentation](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events).

The browser updates keyed task rows without page reloads. It retains selection,
branch state, list/page scroll offsets, and editor drafts independently of loaded
data. Changed rows briefly highlight without being scrolled into view. Save
controls stay locked until the committed snapshot has been refreshed; a second
edit cannot be discarded in the gap between the write and read-back.

`list_revisions` and SQLite triggers maintain a monotonically increasing revision
for each project/date, including empty lists. Items and the revision are read in
one SQLite read transaction. A whole-list save uses `BEGIN IMMEDIATE`, checks the
loaded revision, then upserts, soft-deletes missing items, cleans orphaned
metadata, and advances the revision in the same transaction. A stale clone cannot
overwrite, resurrect, or delete newer work. Empty-list markers prevent deleted
lists from being reimported from old markdown. Multi-project moves and
archive-plus-rollover also commit atomically. Historical archive/fallback reads
use one snapshot.

REST handlers delegate to `todo::ops`, as CLI and MCP do. State changes use
`TodoItem::set_state`, including completion/cancellation timestamps. TUI and plugin
state transitions use the same method. The TUI holds its own persistent
`data_version` connection, defers refresh during editing/unsaved changes, and
preserves the selected task when refreshing. Undo history is cleared after an
external refresh; undo of your own commits inherits the current loaded revision
and still conflicts if another writer has committed since then.

A conflict returns HTTP 409 and preserves the browser draft. Review the displayed
latest task, choose whether to keep the draft or use the latest version, then
explicitly save again. If the task was deleted, the draft can be restored as a new
task. The check is intentionally at list granularity: even an unrelated change in
the same list can require confirmation. No silent last-write-wins merging occurs.
In the TUI a conflict leaves the app and unsaved list intact. F5 writes a
`recovery-<uuid>.md` copy (including the inline edit buffer) to the data directory
and reloads, so changes can be reapplied. Restart all interfaces on the new binary
after upgrading; older binaries do not implement revision checking.

SQLite is authoritative. Markdown export takes a short SQLite writer lock,
reads the latest committed rows, and atomically replaces the file using a unique
temporary file. A slower exporter cannot replace a newer export with an older
snapshot. SQLite and the filesystem cannot share a transaction: an export failure
is logged after the database commit and retried on the next save, without reporting
a failed task creation that could encourage duplicate retries.

## Today and history

Today is based on the **server's** local date, not the browser's clock. The web
workspace uses the shared CLI/MCP automatic rollover-on-load path: if Today does
not yet exist, it carries open tasks and necessary ancestors from the newest prior
list, archives the source, assigns fresh task IDs, and preserves order/hierarchy.
The former CLI/MCP 30-day search limit is removed in favor of the existing newest
prior-list lookup. An already existing empty Today remains empty. The TUI keeps
its configured ask/automatic rollover preference, using the same atomic rollover
operation when approved.

At midnight, the shared observer invalidates Today even without a database write.
Browser mutations send `X-Totui-Today` with their snapshot date; the server rejects
a mismatched date with 409, covering the interval before midnight notification.
The next snapshot rolls forward as needed. A draft from yesterday stays intact
and is marked as belonging to an earlier day; it is never silently written into
today's new task IDs. An explicitly chosen historical date stays fixed and is
read-only in the web interface, using archived rows first and existing active
rows as the fallback. Legacy REST clients can still explicitly mutate dates as
before; the web editor only writes its current, editable Today.

## HTTP compatibility

Existing `/api/health`, `/api/projects`, `/api/todos`, and
`/api/todos/{id}` routes remain. Todo responses retain the existing fields,
including null optional values, and add priority/state descriptions. List
responses add `today`, `project`, `revision`, and `read_only`.

- `GET /api/snapshot?project=default` reads Today; add `date=YYYY-MM-DD` for history.
- `GET /api/events` streams invalidations.
- `POST /api/todos?project=default` accepts the shared creation spec, including
  `parent_id`, `state`, and `priority`.
- `PATCH /api/todos/{id}` accepts `expected_revision`. Omitted optional fields are
  unchanged; `due_date: null`, `priority: null`, or `description: null` clears them.
  Empty descriptions still clear. `clear_due_date: true` and `clear_priority: true`
  are also supported, including by MCP `update_todo` and CLI JSON specs.
- `POST /api/todos/{id}/move` takes `parent_id` (null for top-level), optional
  `before_id` (a sibling in that destination), and `expected_revision`. The whole
  subtree moves; cycles are rejected.
- `DELETE /api/todos/{id}?revision=N` atomically deletes the task and descendants.

Legacy clients can omit the expected revision; their individual read/modify/save
still has persistence-level conflict protection. Clients holding their own drafts
should send a revision to protect edits made before the HTTP request began.
Blank content and invalid states/priority now receive consistent validation
errors instead of bypassing shared rules. Permissive cross-origin CORS has been
removed; same-origin browser writes are checked against Origin/Host. Non-browser
CLI/MCP clients do not need browser CORS headers.

## Measured latency

On 2026-09-08, macOS ARM64, debug Rust binary, local SQLite, and Chromium, eight
separate Python-process SQLite commits reached the next browser animation frame
in **44, 144, 115, 129, 108, 135, 115, and 154 ms** (median 122 ms; maximum
154 ms). All met the 500 ms target. The Playwright test records the clock immediately
after `commit()` returns and observes the changed task DOM followed by
`requestAnimationFrame`. A prior interactive-browser DOM sample of twelve
commits ranged from 35 to 138 ms. These are local measurements, not guarantees for
background tabs, suspended devices, an overloaded database, or a remote proxy.
The automated browser test prints fresh measurements and enforces the 500 ms
ceiling for each sample.

## Future hosting boundary

Static files, API, and SSE are composed in one Axum router, with middleware outside
all routes so authentication can cover the entire surface. The release is for
local use: it does not add OAuth, accounts, public deployment, offline editing,
or synchronization between independent databases. Browsers connected to this
server share its one SQLite database.

For a future HTTPS/OAuth deployment, preserve relative URLs and place trusted
origin/authentication middleware around the whole router. Explicitly configure
external origins and trusted proxy headers; do not blindly trust forwarded Host.
Disable proxy buffering and response transformation/compression for SSE, use a
read timeout comfortably above the ten-second keepalive (for example 60 seconds),
and permit long-lived streaming responses. SSE sends `X-Accel-Buffering: no` and
`Cache-Control: no-cache, no-transform`; the proxy must honor that configuration.
Handle session expiry by closing the stream and authenticating before reopening.
Terminate HTTPS at a trusted proxy and expose only the intended bind interface.

Select a task to use **Organize task** at the top of its editor. Choose another
task in **Move under** to nest it, or use **Move to root** to unnest it. **Position**
places the branch before a sibling or at the end; moves carry all descendants.
Moving into a collapsed destination expands its ancestor path. Use the arrows
beside parent tasks for individual branches, or **Expand all / Collapse all**
above the list. All controls work with touch and keyboard.

## Saving placement and debugging requests

**Save task** saves all editor changes, including **Move under** and **Position**,
in a single revision-checked database save. An invalid move rejects the entire
save; it does not partially update the text. The separate **Move task & descendants**
button remains available for placement-only changes. Unsaved placement is retained
with the browser draft if another interface changes the list.

Run `cargo run --bin totui -- web --open --verbose` to log parsed create, update,
and move payloads plus operation errors alongside request status and latency.
For managed servers, use `RUST_LOG=info,to_tui::api=debug,tower_http=debug` when
starting the daemon. Payload logs include task text and descriptions; they are
opt-in. A normal Save with a parent change sends a PATCH containing
`"placement":{"parent_id":"UUID","before_id":null}`; `parent_id:null` means root.
Omitting `placement` leaves hierarchy and ordering unchanged.

## Development server recipes

```sh
just dev-web --open --verbose
just dev-web --detach --open --verbose --port 48379
just dev-web --detach --restart
just dev-web-status
just dev-web-stop
```

`dev-web` forwards arguments to `cargo run --bin totui -- web`; it opens a browser
only when `--open` is supplied. `--detach` is handled by the development wrapper,
which returns after launching Cargo without waiting for compilation or server
startup. Use status to distinguish starting/building from running, and inspect
`target/dev-web/server.log` for startup failures or verbose payloads. Stopping
terminates the detached process group, including Cargo during compilation.
Status and stop manage only the detached instance recorded by this checkout;
a foreground instance is stopped with Ctrl+C.
Use `--restart` to stop the recorded instance first, wait for it to exit, and start
with the supplied options. If none is running, it starts normally. This also works
without `--detach` to restart in the foreground.
Each detached start replaces the previous log.

The wrapper uses Python 3 and POSIX process groups (macOS/Linux). Its state is
separate from the installed `totui serve` daemon. `TOTUI_DEV_WEB_DIR` overrides the
state/log directory for isolated checks; `TOTUI_DATA_DIR` and other server settings
are inherited. Run `python3 scripts/test_dev_web.py` from the repository root to
verify the recipes against temporary data and an ephemeral port.

## Dragging tasks

Use the six-dot handle at the right of a task to drag with a mouse, touch, or pen.
Drop near another row's top or bottom edge to place the branch before or after it
as a sibling; drop in the middle to nest it under that task. The floating label and
highlight show the exact action before you release. Drop in **Root level** at the
bottom of the list to unnest and append the branch. All descendants move together.
Dragging near the viewport edges scrolls the list; touching the task text still
scrolls normally. Escape or a cancelled touch cancels the move.

Live refreshes wait until the gesture finishes so targets stay still. Each drop
uses the shared revision-checked move operation: if another interface commits
during the gesture, the move is rejected, fresh data is loaded, and a retry message
is shown. Editor drafts remain intact. Historical views have no drag handles.
The existing **Move under**, **Position**, and **Move to root** controls remain
available for keyboard use or whenever dragging is inconvenient.

## State indicators and quick completion

Task rows and the editor use ⬜ pending, 🔄 in progress, ✅ done, ❔ question,
❗ important, and 🚫 cancelled. Click or tap a row's state icon to mark that task
done; click a done task again to return it to pending. Keyboard users can focus
the checkbox and press Space or Enter. The control has a 44px touch target and
an accessible checked state. It changes only that task, preserving descendants
and any unsaved editor draft. All six states remain available in the editor.
Historical checkboxes are disabled. Completion uses the shared timestamp and
revision checks, and propagates through the same live updates as other edits.

Right-click a task row or its checkbox to open the state dropdown. The current
state is marked; choosing any of the six states saves immediately. Keyboard users
can open it with Shift+F10 or the context-menu key, navigate with arrow keys, and
select with Enter or Space. Escape or clicking outside dismisses it. The menu is
unavailable in read-only history. A concurrent change after opening the menu
rejects the stale selection and leaves a visible error; reopen it to retry against
the latest task. Unsaved editor drafts remain intact.

Task rows keep the state in the emoji checkbox instead of repeating a state line.
P0/P1/P2 appear as contrasting inline badges; due dates appear only when set.
Single-line rows are 46px high with 44px controls. On desktop, the task list fills
the space beside the project sidebar until a task is opened. Closing the editor
reclaims that space while preserving its draft. Phones retain the full-screen
editor when a task is opened.


## Managing projects

Choose **Manage projects** in the top bar to create a project, rename it, or delete
it. Project selection is available in the desktop sidebar and the mobile picker.
Renaming preserves tasks, history, and folder bindings; open browser tabs follow
the new name. Deleting requires confirmation and removes the project's tasks,
history, metadata, and saved files. Tabs viewing a deleted project return to
`default`. The default project cannot be renamed or deleted.

The API accepts `POST /api/projects` and `PATCH /api/projects/{id}` with
`{"name":"Work"}`, and `DELETE /api/projects/{id}`. IDs come from
`GET /api/projects`. Duplicate names return 409; invalid names and attempts to
rename or delete `default` return 400; missing project IDs return 404.
