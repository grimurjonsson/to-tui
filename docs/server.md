# Linux server installation

Run one native to-tui service on a Linux VPS with systemd and tzdata installed.
SQLite and the web assets are included in the binary; Docker, PostgreSQL and Node
are not required. Local mode uses the existing invisible local user. Authenticated mode creates a
private workspace for each verified account.

Build/install the CLI with `cargo install --path . --locked`, or use a release
binary matching the VPS architecture. Preview the service without root:

```sh
totui server install --dry-run --timezone Europe/Oslo
```

Install using the interactive wizard:

```sh
sudo /path/to/totui server install
```

`totui server wizard` is an alias for `totui server install`. The wizard asks for
the local proxy port and timezone. For automation, supply options and `--yes`:

```sh
sudo /path/to/totui server install --yes --port 48372 --timezone Europe/Oslo
```

The installer copies the currently running binary to
`/usr/local/lib/totui/totui`, writes `/etc/systemd/system/totui.service`, reloads
systemd, enables the service at boot, starts it, and checks database readiness.
It does not import the invoking user's local lists. A systemd-managed unprivileged
identity owns the persistent state directory at `/var/lib/totui`. With
`DynamicUser`, systemd may store this under `/var/lib/private/totui` and expose a
symlink at `/var/lib/totui`.

The service listens at `127.0.0.1:48372` by default. The timezone controls daily
list dates and should match how you want midnight rollover to behave.

```sh
totui server status
sudo totui server logs --follow
sudo totui server restart
sudo totui server stop
sudo totui server start
```

Logs use the system journal and its retention policy. The installed service
restarts after failures with a five-second delay. A readiness probe checks the
HTTP server and database every ten seconds and notifies systemd's 60-second
watchdog. Startup is limited to 60 seconds. Repeated failures are bounded by
systemd's start limit; inspect the journal and use `sudo systemctl reset-failed
totui.service` after correcting a persistent failure.

`totui server run --port 48372` runs in the foreground for diagnostics. Outside
systemd, it uses the usual local data directory unless `TOTUI_DATA_DIR` is set.
`totui web start/stop/status` manage local desktop servers; use `totui server`
commands for the installed system service.

## Reverse proxy and Google authentication

Configure the existing HTTPS reverse proxy to forward to
`http://127.0.0.1:48372` (or the selected port). Enforce authentication and an
explicit allowed-user policy for browser routes, including the web `/api/` routes.
The `/api/remote/` exception described below uses application-verified client tokens.
Enable `--auth` on the server to verify sessions and isolate user data. The
installer does not modify your proxy or OAuth configuration.

Preserve the public `Host` header so the existing same-origin write check accepts
browser edits. Disable response buffering for `/api/events` and allow long-lived
SSE connections. `/api/health` checks HTTP liveness; `/api/ready` additionally
checks that the database's projects table can be read. Both can remain behind
Google authentication externally; the local watchdog accesses them directly.

A proxy running inside a container cannot reach the host service through its own
loopback address. Adapt the proxy's host networking before using this deployment;
do not simply expose the unauthenticated application publicly.

Authentication is opt-in on every server entry point:

```sh
totui web --auth
totui web start --auth
totui serve start --auth
totui server run --auth
sudo /path/to/totui server install --auth --yes
```

Without `--auth`, the TUI, CLI, MCP and local web workspace use the existing data
under `TOTUI_DATA_DIR` (or `~/.to-tui`) as the implicit `local` user. Login is not
required, and cookies/identity headers do not select a different local user.
Managed web restarts and systemd installation replacements preserve enabled
authentication. To deliberately return to local web mode, stop the managed server
and start it again without `--auth`.

With `--auth`, the application sends the request's session cookie to the configured
OAuth auth-check endpoint. `TOTUI_AUTH_URL` defaults to
`http://127.0.0.1:4180/oauth2/auth`; use a systemd `Environment=` drop-in to configure
another endpoint for the installed service. This endpoint is trusted to validate
sessions and must return HTTP 202 with `X-Auth-Request-User` containing the stable
provider subject (`sub` for Google). oauth2-proxy needs `set_xauthrequest = true`.
The app does not trust identity headers supplied by the client. Missing identity,
invalid sessions, redirects or gateway failures never fall back to local data.

Accounts are keyed by authentication endpoint and provider subject, not email.
Emails are optional display metadata and may change without creating a new user.
Keep the authentication endpoint stable when upgrading. Verified accounts are
provisioned on first access in `users.db`; their databases, exports, project
configuration and archives live under `users/<UUID>/`. `/api/me` returns only the
current user's ID and email. No user-list or cross-user administration API is
exposed. API responses are not cacheable, and SSE notifications are tenant-specific.
The browser sends its expected user ID so an account switch cannot save a stale
draft into the next user's workspace.

`/api/health` and `/api/ready` are identity-free process checks with no user data;
the external Nginx site still protects them. Back up the entire data directory,
including `users.db` and `users/`. Existing local/shared data remains under the
implicit local user and is never assigned to the first person who signs in.

Starting the TUI on an older installation automatically upgrades its SQLite
schema in place. Existing tasks and archives stay in the local workspace without
login or account setup. Schema changes run in a transaction: failures are reported
and rolled back so startup can be retried. Legacy `dailies/` files move into
`projects/default/dailies/`; interrupted moves resume on the next startup, and
conflicting files are preserved in both locations without overwriting either.

## Connect a desktop client

Install a build containing remote support on **both** the VPS and your computer.
Older servers return an upgrade error because they lack `/api/remote/v1`.
Use the replacement/backup procedure below to update the service; retain `--auth`
and the existing OAuth proxy. Install the updated checked-in Nginx configuration:
`/remote/login` remains behind Google authentication, while `/api/remote/` is
forwarded directly to the **authenticated** application. The application verifies
client tokens on workspace requests; only code exchange and token revocation run
without a browser session, and both require their respective secret credentials.
Do not use this proxy exception with an application running without `--auth`.

On your computer:

```sh
totui remote add home https://totui.gimmi.is
totui remote login home
totui remote use home
totui
```

`login` opens your default browser (and prints a link if it cannot open one).
Sign in through the existing Google gateway and click **Connect this client**.
The browser returns a one-time authorization code to a temporary listener on
`127.0.0.1`; the CLI exchanges it over HTTPS using a PKCE verifier and stores the
resulting to-tui token. The browser never receives the app token. Cancellation
returns to the CLI, and an unfinished login times out after five minutes.

Tokens expire after 30 days. `totui remote logout home` revokes the token on the
server and removes the saved credential; it does not sign you out of Google.
The client verifies `/api/remote/me`, pins requests to that account, and checks
protocol compatibility before selecting the remote. Token files are stored under
`~/.to-tui/remote-credentials/` with a private directory and mode-0600 files on Unix.
The normal config contains only the URL, expected account ID, and default selection.
The server stores only token hashes in `users.db`; include that file in backups.

For legacy deployments, `totui remote login home --cookie-stdin` still imports a
browser Cookie header from stdin. Normal browser login does not require copying
cookies or creating another Google OAuth application.

```sh
totui remote status home
totui remote list
totui --remote home                 # one launch without changing the default
totui --remote home show
totui --remote home add "Server task"
totui --local                       # one launch using local todos
totui remote local                  # restore local as the default
totui remote logout home            # forget the session, retain selection
totui remote remove home
```

The TUI supports remote edits, nested tasks, undo/redo, project management, atomic
moves between projects, archive viewing, and rollover. Task reads and edits use a
persistent SQLite cache. Queued edits upload in the background, while an authenticated
SSE connection delivers changed tasks and ETags immediately after the server's
shared database watcher detects a commit (normally within 100 ms, plus network
latency). There is no periodic full-snapshot download. After the first successful connection, the TUI
can start and edit offline. Edits and in-flight request IDs survive exit or crashes;
reopening the same remote workspace resumes syncing. The status bar shows queued
edits, sync errors, and conflicts. F6 opens conflicts or displays the latest error.
Only one TUI may open the same server/account cache at a time.

Concurrent edits to the same task always require a decision, including edits to
different fields. Choose `l` to keep your version, `s` to keep the server version,
or `m` to combine fields manually. In combine mode, use up/down to review fields,
`l`/`s` to choose each field, and Enter to save. Every differing field needs an
explicit choice. Esc leaves the conflict pending; F6 reopens it. New server edits
during resolution require another review. Changes to separate tasks sync
automatically. F5 saves a local recovery copy and reloads the cached list.

Project creation/rename/deletion and rollover currently require a connection and
a fully synced queue. These infrequent operations still wait for the server.
CLI `add`, `show`, and `todo` commands access the server directly. Ordinary requests time out after
ten seconds; cached TUI edits remain queued when the server is unavailable.

Remote UI preferences, logs, and recovery files are isolated by server/account
under `~/.to-tui/remote-workspaces/`; existing local todo data remains separate.
`TOTUI_DATA_DIR` relocates these directories along with normal local data.
HTTPS is required, with HTTP allowed only for loopback development servers.
Client and server calendar dates should match for rollover. Queued task edits
retain their original date when uploaded later. Server management commands run on the server itself.
Other CLI commands require explicit `--local` when a default remote is selected.
The bundled skills use the CLI/API, not the legacy MCP backend. That standalone
MCP binary remains local-only; it must not be used for a selected remote workspace.
Remote plugin metadata operations are not supported yet.

For automation, run `totui todo context` from the user's active project directory
before writing. It returns the resolved backend, remote name/server URL, folder,
and project. `totui todo` now follows the TUI's folder mapping and fallback unless
`--project NAME` is explicit. Skills must announce the destination before creating
a tree and confirm where it landed afterward. Pin it in subsequent commands:

```sh
totui todo context
totui --remote home todo list --project to-tui
totui --remote home todo create --project to-tui --content "Verify reconnects"
totui --remote home todo update UUID --project to-tui --state x
```

JSON results remain on stdout; mutation destinations and errors go to stderr.
Remote failure never falls back to the local database. `--local` explicitly selects
local data. Both `skills/totui` and `skills/todo-mcp` use this workflow (the latter
retains its old name for compatibility).

The first connection downloads a full snapshot. SSE resumes with a durable
`Last-Event-ID` cursor scoped to the account's database. The client persists its
cursor with the cached changes, so reconnects replay only changes since that
cursor. The journal retains the last 100,000 row-change entries across server
restarts. An expired/foreign cursor, database rollback, or a catch-up larger than
8 MiB requests a fresh snapshot. Pending edits survive that resync and still use
the same conflict policy. Existing caches upgrade with one initial resync.

`GET /api/remote/events` sends `sync` events containing task upserts/tombstones,
changed archive lists, and project/date indexes when those indexes change.
`GET /api/remote/changes?since=EPOCH:SEQUENCE` supports the same incremental catch-up.
`POST /api/remote/sync?since=EPOCH:SEQUENCE` returns changes as the upload
acknowledgement. Legacy callers without `since` retain snapshot responses.
Normal remote TUI synchronization uses these incremental responses.

The TUI commits edits to its local SQLite cache before background upload. Cache
records are stored separately, so a checkbox change writes only the changed task
and sync metadata instead of serializing the entire workspace. Transactions use
WAL with full synchronization; failed commits retain the previous cache state.
Older single-record caches migrate transactionally on first open, preserving
queued uploads, conflict choices, history, and the sync cursor. Older TUI builds
cannot read the migrated format; use an updated binary or restore a matching
pre-upgrade cache backup when rolling back.

Each active account shares a database observer across SSE connections, including
changes committed by other processes. Watch notifications coalesce under load;
streams do not accumulate an unbounded event queue or hold a database transaction
open while waiting. Heartbeats arrive every ten seconds. Streams close every minute
and reconnect to revalidate authentication, with capped exponential backoff and
jitter after connection failures. Nginx must disable response buffering and use an
idle timeout longer than the heartbeat interval; the included configuration does.

The versioned remote API exposes typed workspace operations, not SQL or arbitrary
paths. It uses the existing authenticated tenant context, expected-account checks,
and list revisions for legacy clients. `/api/remote/sync` provides the task feed and
atomic conditional batches with durable idempotency receipts. Task versions are
advanced by database triggers, including writes from the web UI and MCP. Deleted
tasks retain version tombstones. `/api/remote/items/{id}` exposes strong `ETag`
headers; PUT/DELETE require `If-Match` (creation uses `If-None-Match: *`). Stale
versions return HTTP 412; absent preconditions return HTTP 428. Batch mutations
carry the equivalent `if_match` token per task. Task saves and moves commit in one
transaction; file paths are derived on the server rather than supplied by clients.

## Updates, backups and removal

Stop the service and take a consistent backup of its entire state directory
before upgrading. For example, run these as separate commands and check each
succeeds:

```sh
sudo systemctl stop totui.service
sudo tar --dereference -C /var/lib -czf /root/totui-backup.tar.gz totui
sudo systemctl start totui.service
```

Use a distinct backup filename each time, keep a copy off the VPS, and periodically
verify restoration. To restore, stop the service and restore the complete backed-up
data into the state directory, preserving ownership and permissions, before
starting it. A binary downgrade may require restoring its matching pre-upgrade
data because startup migrations can change the schema.

From a checkout on the VPS, update to the latest stable GitHub release with:

```sh
just upgrade-server-with-curl
```

Run this as your normal user; only backup and installation commands use sudo.
The command checks noninteractive sudo access at startup and again before making
changes. It never prompts for a sudo password: cached credentials or passwordless
sudo are required. If needed, authenticate separately with `sudo -v`; if you have
forgotten your password, an administrator must restore access or configure sudo.
Running as root does not require sudo.

The command requires Python 3, curl, tar, sudo when run as a non-root user, and an
installed, enabled, running managed service. It checks the Linux architecture and installed version, then
prompts with the new and installed versions in color. Enter `y` to proceed;
Enter alone cancels. An equal or newer installed version is left alone.
The release asset must have finished uploading and provide a SHA-256 digest.
The command verifies the download and its version before stopping the service,
backs up the data, installed binary, and unit under a unique `/root/totui-backup.*`
directory, then invokes the replacement installer. `data.tar.gz` contains the
entire `/var/lib/totui` tree, including `todos.db`, all per-user databases,
`users.db` (accounts and token hashes), and any SQLite WAL files present. The
service is stopped while this archive is created to keep the databases consistent.
It prints the backup directory and the exact `sudo rm -rf -- /root/totui-backup.…`
command for deleting that backup manually. Backups are never automatically removed;
copy them off the VPS or delete them when no longer needed. A failed backup restarts the unchanged service. A failed
upgrade reports the backup location and leaves rollback to the operator.

Alternatively, update using a new binary you downloaded yourself:

```sh
sudo /path/to/new/totui server install --replace --yes
```

Replacement preserves data, atomically replaces the binary and unit, and restarts
the service. Omitted port and timezone settings retain their previous values;
pass `--port` or `--timezone` to change them. Backups and rollback are operator-managed in this first version.
Systemd drop-ins are preserved and can override generated settings.

To remove the service, disable it and remove the installed files:

```sh
sudo systemctl disable --now totui.service
sudo rm /etc/systemd/system/totui.service
sudo rm /usr/local/lib/totui/totui
sudo systemctl daemon-reload
```

These commands retain the server data. The installer does not automatically
configure backups, TLS, firewall rules, or OAuth.

## totui.gimmi.is deployment

The VPS uses the site configuration in
[`deploy/nginx/totui.gimmi.is.conf`](../deploy/nginx/totui.gimmi.is.conf).
Nginx terminates HTTPS and forwards authenticated requests to the native service
at `127.0.0.1:48372`. The Google gateway remains the existing oauth2-proxy instance
at `127.0.0.1:4180`, with the existing allowed-user file. The installed service runs with `--auth`, with a private workspace for each
verified account and no IP-based authentication bypass.

The gateway's existing callback is `https://cait.gimmi.is/oauth2/callback`.
Its `.gimmi.is` cookie domain and redirect allowlist allow login to return to
`https://totui.gimmi.is/` without registering a second Google application.
Unauthenticated page requests start Google sign-in; unauthenticated workspace API
requests return HTTP 401. The `/api/remote/` location delegates authentication to
the application so native clients can use their own tokens. Browser login grants
are single-use, bound to the verified Google account and a PKCE challenge, and
can redirect only to a loopback callback port. Live-update connections are forwarded without buffering.

The certificate is managed by the existing Certbot timer. HTTP ACME challenges
use `/var/lib/totui-acme`; other HTTP requests redirect to HTTPS. The deployment
hook in
[`deploy/letsencrypt/totui-nginx-reload`](../deploy/letsencrypt/totui-nginx-reload)
is installed under `/etc/letsencrypt/renewal-hooks/deploy/` and validates/reloads
Nginx when this domain's certificate renews.

The service uses UTC for daily lists and `/var/lib/totui` for persistent storage.
Local desktop and former shared lists are not imported or assigned to Google
accounts by installation. Desktop clients use the authenticated `/api/remote/v1` endpoint after both installations are upgraded.

API handlers execute inside a request-scoped storage root. Storage work dispatched
to a blocking worker uses `storage::context::blocking` to carry that root onto the
worker thread; unscoped workers would use the local user's root. SSE opens its
connection within the same scope and retains that connection only for the active
stream. Tenant routing does not change process environment variables.

## Web account controls and owner upgrades

The bottom of the web sidebar shows the running server version and verified account.
The avatar uses Gravatar with a SHA-256 hash of the trimmed, lowercase verified
email, a G rating, and no referrer header. Missing images and failed image loads
show initials instead; local workspaces make no Gravatar request. Clicking the
avatar or email opens the account menu with **Manage projects** and **Log out**.
A small green check beside the version appears only after a successful release
check confirms that no newer version is available. Logging out
clears the OAuth gateway session and opens a signed-out page. Google remains signed
in. This deployment shares its gateway cookie with other `.gimmi.is` sites, so
logging out also ends that shared gateway session. `/signed-out` must be forwarded
without `auth_request`, as in the checked-in Nginx site configuration.

The server checks the latest stable GitHub release at most once an hour. An **⬆️**
button beside the version highlights an available update. All users can see it;
only the explicitly configured owner can trigger it. Failed release checks are
shown as unavailable rather than claiming the server is up to date.

Web upgrades are disabled by default and unavailable in local mode. On a managed
Linux server, install the root-owned helper and its systemd units:

```sh
sudo install -o root -g root -m 0755 scripts/upgrade-server.py /usr/local/lib/totui/upgrade-server.py
sudo install -o root -g root -m 0644 deploy/systemd/totui-upgrade.service deploy/systemd/totui-upgrade.path /etc/systemd/system/
sudo systemctl edit totui.service
```

Set these values in the drop-in, using the owner's `id` from `/api/me` while
signed in (the UUID, not the email address):

```ini
[Service]
Environment=TOTUI_WEB_UPGRADE=1
Environment=TOTUI_SERVER_OWNER_ID=OWNER-ACCOUNT-UUID
```

Then activate the watcher and reload the server configuration:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now totui-upgrade.path
sudo systemctl restart totui.service
```

The API checks the authenticated account on every upgrade request and requires
the version the owner confirmed. It atomically queues one fixed request file.
The separate root systemd job independently checks the latest release, verifies
the downloaded checksum/version, stops only `totui.service`, and takes the same
complete backup as the CLI upgrade before installation. The web process retains
its existing `NoNewPrivileges` and filesystem restrictions; it gets no sudo access.
No request supplies an executable, command, download URL, or installation path.

Upgrade progress and failures appear beside the sidebar account controls, including
after reconnection. Successful or already-up-to-date results do not show a notice.
Failures are reported there; detailed errors and backup locations are in
`sudo journalctl -u totui-upgrade.service`. Failed installation still requires
operator-managed rollback using the retained backup. To disable web upgrades,
remove `TOTUI_WEB_UPGRADE` from the drop-in, restart the server, and disable
`totui-upgrade.path`. Other websites and the OAuth service are not restarted by
an upgrade.

### Concurrent Google login attempts

The shared OAuth gateway must keep a separate CSRF cookie for each login attempt.
Otherwise, logging out while other tabs reconnect can start overlapping logins:
all use `_cait_oauth2_csrf`, and the later attempt overwrites the earlier cookie.
The earlier Google callback then fails with `CSRF token mismatch` / `Unable to
find a valid CSRF token`, even though the `.gimmi.is` cookie domain is correct.

The VPS gateway configuration at `/etc/oauth2-proxy-cait.cfg` includes:

```toml
cookie_csrf_per_request = true
cookie_csrf_per_request_limit = 8
```

These are supported by its installed oauth2-proxy v7.15.3. Keep the existing
cookie name, domain, secret, and CSRF validation. Validate configuration with
`oauth2-proxy --config /etc/oauth2-proxy-cait.cfg --config-test` before restarting
`oauth2-proxy-cait.service`. The bounded per-request setting is documented in the
[OAuth2 Proxy cookie options](https://oauth2-proxy.github.io/oauth2-proxy/configuration/overview/#cookie-options).

Run `python3 scripts/test_oauth_login.py` against the deployed site to verify that
three overlapping logins preserve the first login cookie and send it to the
callback domain. This test follows no Google redirects, performs no account login,
and prints no cookie values or authorization codes. After fixing this setting,
start a fresh login from the application URL; old callback URLs remain invalid.
