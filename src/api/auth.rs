use super::web::StartupProject;
use crate::storage::context;
use crate::utils::paths::get_to_tui_dir;

use anyhow::{Context, Result, ensure};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use uuid::Uuid;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DEFAULT_AUTH_URL: &str = "http://127.0.0.1:4180/oauth2/auth";

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        context::with_root(root.clone(), crate::storage::ensure_installation_ready)?;
        Ok(Self { root })
    }
}

#[derive(Debug, Clone)]
pub struct ServerState {
    root: PathBuf,
    mode: Mode,
}

#[derive(Debug, Clone)]
enum Mode {
    Local(Workspace),
    Auth {
        url: reqwest::Url,
        client: reqwest::Client,
        workspaces: Arc<Mutex<HashMap<String, Workspace>>>,
    },
}

fn registry(root: &Path) -> Result<Connection> {
    let conn = Connection::open(root.join("users.db"))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

impl ServerState {
    pub fn local() -> Result<Self> {
        let root = get_to_tui_dir()?;
        Ok(Self {
            mode: Mode::Local(Workspace::open(root.clone())?),
            root,
        })
    }

    pub fn authenticated(auth_url: &str) -> Result<Self> {
        let url = reqwest::Url::parse(auth_url).context("Invalid OAuth authentication URL")?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none(),
            "Authentication URL must be an HTTP(S) auth-check endpoint without credentials or a fragment"
        );
        let root = get_to_tui_dir()?;
        fs::create_dir_all(&root)?;
        let conn = registry(&root)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY NOT NULL,
            authority TEXT NOT NULL,
            subject TEXT NOT NULL,
            email TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(authority, subject)
        )",
        )?;
        fs::create_dir_all(root.join("users"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join("users"), fs::Permissions::from_mode(0o700))?;
            fs::set_permissions(root.join("users.db"), fs::Permissions::from_mode(0o600))?;
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()?;
        Ok(Self {
            root,
            mode: Mode::Auth {
                url,
                client,
                workspaces: Arc::default(),
            },
        })
    }

    pub async fn ready(&self) -> bool {
        let state = self.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = match state.mode {
                Mode::Local(_) => Connection::open(state.root.join("todos.db"))?,
                Mode::Auth { .. } => registry(&state.root)?,
            };
            let table = if matches!(state.mode, Mode::Local(_)) {
                "projects"
            } else {
                "users"
            };
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })?;
            Ok(())
        })
        .await
        .is_ok_and(|result| result.is_ok())
    }

    async fn resolve(&self, headers: &HeaderMap) -> Result<(User, Workspace), StatusCode> {
        let Mode::Auth {
            url,
            client,
            workspaces,
        } = &self.mode
        else {
            if let Mode::Local(workspace) = &self.mode {
                return Ok((
                    User {
                        id: "local".into(),
                        email: None,
                    },
                    workspace.clone(),
                ));
            }
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };
        let cookie = headers
            .get(header::COOKIE)
            .filter(|value| !value.is_empty())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let response = client
            .get(url.clone())
            .header(header::COOKIE, cookie.clone())
            .send()
            .await
            .map_err(|error| {
                tracing::warn!(%error, "Authentication service unavailable");
                StatusCode::SERVICE_UNAVAILABLE
            })?;
        if response.status() != StatusCode::ACCEPTED {
            return Err(if matches!(response.status().as_u16(), 401 | 403) {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::BAD_GATEWAY
            });
        }
        let subject = identity_header(response.headers(), "x-auth-request-user", 512)
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let email = identity_header(response.headers(), "x-auth-request-email", 254);
        let root = self.root.clone();
        let authority = url.to_string();
        let workspaces = workspaces.clone();
        tokio::task::spawn_blocking(move || -> Result<(User, Workspace)> {
            let conn = registry(&root)?;
            let existing: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT id, email FROM users WHERE authority=?1 AND subject=?2",
                    params![authority, subject],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let id = if let Some((id, previous_email)) = existing {
                if previous_email != email {
                    conn.execute("UPDATE users SET email=?1 WHERE id=?2", params![email, id])?;
                }
                id
            } else {
                conn.execute(
                    "INSERT INTO users(id, authority, subject, email) VALUES(?1,?2,?3,?4)
                    ON CONFLICT(authority,subject) DO NOTHING",
                    params![Uuid::new_v4().to_string(), authority, subject, email],
                )?;
                conn.query_row(
                    "SELECT id FROM users WHERE authority=?1 AND subject=?2",
                    params![authority, subject],
                    |row| row.get::<_, String>(0),
                )?
            };
            let directory = Uuid::parse_str(&id)
                .context("Invalid stored user ID")?
                .to_string();
            let mut workspaces = workspaces
                .lock()
                .map_err(|_| anyhow::anyhow!("Workspace registry lock failed"))?;
            let workspace = match workspaces.get(&id) {
                Some(workspace) => workspace.clone(),
                None => {
                    let workspace = Workspace::open(root.join("users").join(directory))?;
                    workspaces.insert(id.clone(), workspace.clone());
                    workspace
                }
            };
            Ok((User { id, email }, workspace))
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|error| {
            tracing::error!(%error, "Could not open user workspace");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}

fn identity_header(headers: &HeaderMap, name: &str, max_length: usize) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    (!value.is_empty()
        && value.len() <= max_length
        && value.trim() == value
        && !value.chars().any(char::is_control))
    .then(|| value.to_owned())
}

pub async fn identify(
    State(state): State<ServerState>,
    mut request: Request,
    next: Next,
) -> Response {
    if matches!(request.uri().path(), "/api/health" | "/api/ready") {
        return next.run(request).await;
    }
    let (user, workspace) = match state.resolve(request.headers()).await {
        Ok(resolved) => resolved,
        Err(status) => return status.into_response(),
    };
    let user_header = axum::http::HeaderValue::from_str(&user.id);
    let Ok(user_header) = user_header else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if request
        .headers()
        .get("x-totui-expected-user")
        .is_some_and(|expected| expected != user_header)
    {
        let mut response = (
            StatusCode::CONFLICT,
            "The authenticated account changed; reload before saving",
        )
            .into_response();
        response.headers_mut().insert("x-totui-user", user_header);
        return response;
    }
    if matches!(state.mode, Mode::Auth { .. }) {
        request
            .extensions_mut()
            .insert(StartupProject("default".into()));
    }
    request.extensions_mut().insert(user);
    let mut response = context::scope(workspace.root, next.run(request)).await;
    response.headers_mut().insert("x-totui-user", user_header);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}

pub async fn me(axum::Extension(user): axum::Extension<User>) -> axum::Json<User> {
    axum::Json(user)
}
