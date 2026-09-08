use super::auth::{ServerState, User};
use anyhow::{Result, ensure};
use axum::{
    Extension, Form, Json,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Duration;

pub fn secret() -> String {
    (0..3)
        .map(|_| uuid::Uuid::new_v4().simple().to_string())
        .collect()
}

pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS client_grants (
        code_hash TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id),
        challenge TEXT NOT NULL, state TEXT NOT NULL, port INTEGER NOT NULL,
        approved INTEGER NOT NULL DEFAULT 0, expires INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS client_tokens (
        token_hash TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), expires INTEGER NOT NULL
    );")?;
    Ok(())
}

fn database(root: &Path) -> Result<Connection> {
    let conn = Connection::open(root.join("users.db"))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

pub(crate) fn verify(root: &Path, token: &str) -> Result<User> {
    ensure!(
        token.starts_with("totui_") && token.len() == 102,
        "Invalid client token"
    );
    let conn = database(root)?;
    Ok(conn.query_row("SELECT u.id,u.email FROM client_tokens t JOIN users u ON u.id=t.user_id WHERE token_hash=?1 AND expires>?2", params![challenge(token), Utc::now().timestamp()], |row| Ok(User { id: row.get(0)?, email: row.get(1)? }))?)
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginQuery {
    pub challenge: String,
    pub state: String,
    pub port: u16,
}

fn validate_query(query: &LoginQuery) -> Result<()> {
    ensure!(query.port >= 1024, "Invalid callback port");
    ensure!(
        query.challenge.len() == 43
            && query
                .challenge
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c)),
        "Invalid PKCE challenge"
    );
    ensure!(
        query.state.len() == 96 && query.state.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid login state"
    );
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn page(body: String) -> Response {
    let mut response = Html(format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Connect to-tui</title><style>body{{font:18px system-ui;background:#10141d;color:#e8edf5;margin:0;display:grid;place-items:center;min-height:100vh}}main{{max-width:440px;padding:36px}}p{{line-height:1.6;color:#b9c5d8}}button{{font:inherit;border:0;border-radius:8px;padding:12px 20px;cursor:pointer;background:#86e3b5;color:#10141d;margin:8px 8px 0 0}}button[value=cancel]{{background:#303949;color:#e8edf5}}</style><main>{body}</main></html>")).into_response();
    response.headers_mut().insert(
        "content-security-policy",
        "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'"
            .parse()
            .expect("Static CSP"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

pub async fn login(
    Extension(server): Extension<ServerState>,
    Extension(user): Extension<User>,
    Query(query): Query<LoginQuery>,
) -> Response {
    if validate_query(&query).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let root = match server.client_auth_root() {
        Ok(root) => root,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let consent = secret();
    let code_hash = challenge(&consent);
    let id = user.id.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = database(&root)?;
        let now = Utc::now().timestamp();
        conn.execute("DELETE FROM client_grants WHERE expires<=?1", [now])?;
        conn.execute("DELETE FROM client_tokens WHERE expires<=?1", [now])?;
        conn.execute("INSERT INTO client_grants(code_hash,user_id,challenge,state,port,expires) VALUES(?1,?2,?3,?4,?5,?6)", params![code_hash,id,query.challenge,query.state,query.port,now+300])?;
        Ok(())
    }).await;
    if !matches!(result, Ok(Ok(()))) {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    page(format!(
        "<h1>Connect to-tui</h1><p>Signed in as <strong>{}</strong>.</p><p>Allow the to-tui client on this computer to view and edit your todos? Only continue if you just ran <code>totui remote login</code>.</p><form method=\"post\" action=\"/remote/login\"><input type=\"hidden\" name=\"consent\" value=\"{}\"><button name=\"action\" value=\"approve\">Connect this client</button><button name=\"action\" value=\"cancel\">Cancel</button></form>",
        escape(user.email.as_deref().unwrap_or(&user.id)),
        consent
    ))
}

#[derive(Debug, Clone, Deserialize)]
pub struct Consent {
    pub consent: String,
    pub action: String,
}

pub async fn approve(
    Extension(server): Extension<ServerState>,
    Extension(user): Extension<User>,
    Form(input): Form<Consent>,
) -> Response {
    let root = match server.client_auth_root() {
        Ok(root) => root,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if input.consent.len() != 96 || !matches!(input.action.as_str(), "approve" | "cancel") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let result = tokio::task::spawn_blocking(move || -> Result<String> {
        let mut conn = database(&root)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let record: Option<(String,String,u16)> = tx.query_row("SELECT challenge,state,port FROM client_grants WHERE code_hash=?1 AND user_id=?2 AND approved=0 AND expires>?3", params![challenge(&input.consent),user.id,Utc::now().timestamp()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (pkce,state,port) = record.ok_or_else(|| anyhow::anyhow!("Expired or invalid login"))?;
        tx.execute("DELETE FROM client_grants WHERE code_hash=?1", [challenge(&input.consent)])?;
        let mut callback = reqwest::Url::parse(&format!("http://127.0.0.1:{port}/callback"))?;
        callback.query_pairs_mut().append_pair("state", &state);
        if input.action == "approve" {
            let code = secret();
            tx.execute("INSERT INTO client_grants(code_hash,user_id,challenge,state,port,approved,expires) VALUES(?1,?2,?3,?4,?5,1,?6)", params![challenge(&code),user.id,pkce,state,port,Utc::now().timestamp()+60])?;
            callback.query_pairs_mut().append_pair("code", &code);
        } else { callback.query_pairs_mut().append_pair("error", "access_denied"); }
        tx.commit()?;
        Ok(callback.into())
    }).await;
    match result {
        Ok(Ok(url)) => {
            let mut response = Redirect::to(&url).into_response();
            response.headers_mut().insert(
                "referrer-policy",
                axum::http::HeaderValue::from_static("no-referrer"),
            );
            response
        }
        _ => (
            StatusCode::BAD_REQUEST,
            "Login expired or was already used. Run totui remote login again.",
        )
            .into_response(),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Exchange {
    pub code: String,
    pub verifier: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Token {
    pub access_token: String,
    pub expires_at: i64,
}

pub async fn exchange(
    Extension(server): Extension<ServerState>,
    Json(input): Json<Exchange>,
) -> Response {
    let root = match server.client_auth_root() {
        Ok(root) => root,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if input.code.len() != 96
        || !(43..=128).contains(&input.verifier.len())
        || !input
            .verifier
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._~".contains(&c))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let result = tokio::task::spawn_blocking(move || -> Result<Token> {
        let mut conn = database(&root)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let id: String = tx.query_row("DELETE FROM client_grants WHERE code_hash=?1 AND challenge=?2 AND approved=1 AND expires>?3 RETURNING user_id", params![challenge(&input.code),challenge(&input.verifier),Utc::now().timestamp()], |row| row.get(0))?;
        let access_token = format!("totui_{}", secret());
        let expires_at = Utc::now().timestamp() + 30*24*60*60;
        tx.execute("INSERT INTO client_tokens(token_hash,user_id,expires) VALUES(?1,?2,?3)", params![challenge(&access_token),id,expires_at])?;
        tx.commit()?;
        Ok(Token { access_token, expires_at })
    }).await;
    let mut response = match result {
        Ok(Ok(token)) => Json(token).into_response(),
        _ => StatusCode::UNAUTHORIZED.into_response(),
    };
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

pub async fn revoke(Extension(server): Extension<ServerState>, headers: HeaderMap) -> Response {
    let root = match server.client_auth_root() {
        Ok(root) => root,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match tokio::task::spawn_blocking(move || -> Result<()> {
        database(&root)?.execute(
            "DELETE FROM client_tokens WHERE token_hash=?1",
            [challenge(&token)],
        )?;
        Ok(())
    })
    .await
    {
        Ok(Ok(())) => Json(serde_json::Value::Null).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
