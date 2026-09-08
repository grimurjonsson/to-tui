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
explicit allowed-user policy for the entire site, including every `/api/` route.
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

Remote CLI/TUI connections and `totui remote login NAME` are not implemented yet.

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

Update using the new binary:

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
Unauthenticated page requests start Google sign-in; unauthenticated API
requests return HTTP 401. Live-update connections are forwarded without buffering.

The certificate is managed by the existing Certbot timer. HTTP ACME challenges
use `/var/lib/totui-acme`; other HTTP requests redirect to HTTPS. The deployment
hook in
[`deploy/letsencrypt/totui-nginx-reload`](../deploy/letsencrypt/totui-nginx-reload)
is installed under `/etc/letsencrypt/renewal-hooks/deploy/` and validates/reloads
Nginx when this domain's certificate renews.

The service uses UTC for daily lists and `/var/lib/totui` for persistent storage.
Local desktop and former shared lists are not imported or assigned to Google
accounts by installation. Remote CLI/TUI support remains separate from this web deployment.

API handlers execute inside a request-scoped storage root. Storage work dispatched
to a blocking worker uses `storage::context::blocking` to carry that root onto the
worker thread; unscoped workers would use the local user's root. SSE opens its
connection within the same scope and retains that connection only for the active
stream. Tenant routing does not change process environment variables.
