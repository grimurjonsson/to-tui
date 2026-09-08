pub mod cache;
pub mod events;
pub mod login;
pub mod protocol;
mod server;
pub mod sync_api;

pub use server::handle;

use crate::api::auth::User;
use crate::remote::protocol::{Request, Snapshot};
use crate::todo::TodoList;
use anyhow::{Context, Result, bail, ensure};
use reqwest::header::{COOKIE, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub url: String,
    pub user_id: Option<String>,
}

#[derive(Clone)]
pub struct Client {
    config: RemoteConfig,
    cookie: Option<HeaderValue>,
    token: Option<HeaderValue>,
    cache: Option<Arc<cache::Cache>>,
}

static CLIENT: OnceLock<Client> = OnceLock::new();

pub fn active() -> Option<Client> {
    if crate::storage::context::data_root().is_some() {
        return None;
    }
    CLIENT.get().cloned()
}

pub fn activate(client: Client) -> Result<()> {
    CLIENT
        .set(client)
        .map_err(|_| anyhow::anyhow!("Remote workspace is already selected"))
}

pub fn validate_url(value: &str) -> Result<String> {
    let url = reqwest::Url::parse(value).context("Invalid remote URL")?;
    ensure!(
        url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
        "Remote URL requires HTTPS (HTTP is allowed only on loopback)"
    );
    ensure!(
        url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/",
        "Use a server origin, such as https://totui.gimmi.is, without a path or credentials"
    );
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Remote names may contain letters, numbers, hyphens and underscores"
    );
    Ok(())
}

pub fn credential_path(name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    Ok(crate::utils::paths::get_to_tui_dir()?
        .join("remote-credentials")
        .join(format!("{name}.cookie")))
}

pub fn save_cookie(name: &str, cookie: &str) -> Result<()> {
    Client::cookie(cookie)?;
    let path = credential_path(name)?;
    let parent = path.parent().context("Missing credential directory")?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    file.write_all(cookie.trim().as_bytes())?;
    file.persist(path)
        .context("Could not store remote session")?;
    Ok(())
}

pub fn token_path(name: &str) -> Result<PathBuf> {
    Ok(credential_path(name)?.with_extension("json"))
}

pub fn save_token(name: &str, token: &crate::api::client_auth::Token) -> Result<()> {
    let path = token_path(name)?;
    let parent = path.parent().context("Missing credential directory")?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    file.write_all(&serde_json::to_vec(token)?)?;
    file.persist(path).context("Could not store client token")?;
    Ok(())
}

pub fn forget_credentials(name: &str) -> Result<()> {
    for path in [credential_path(name)?, token_path(name)?] {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

impl Client {
    pub fn workspace_path(&self, root: PathBuf) -> PathBuf {
        use base64::Engine;
        let key = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!(
            "{}\n{}",
            self.config.url,
            self.config.user_id.as_deref().unwrap_or("unknown")
        ));
        let mut path = root.join("remote-workspaces");
        for chunk in key.as_bytes().chunks(100) {
            path.push(String::from_utf8_lossy(chunk).as_ref());
        }
        path.join("workspace")
    }

    pub fn url(&self) -> &str {
        &self.config.url
    }

    fn cookie(cookie: &str) -> Result<HeaderValue> {
        ensure!(!cookie.trim().is_empty(), "Session cookie cannot be empty");
        let mut value =
            HeaderValue::from_str(cookie.trim()).context("Invalid session cookie header")?;
        value.set_sensitive(true);
        Ok(value)
    }

    pub fn new(config: RemoteConfig, cookie: Option<&str>) -> Result<Self> {
        validate_url(&config.url)?;
        Ok(Self {
            config,
            cookie: cookie.map(Self::cookie).transpose()?,
            token: None,
            cache: None,
        })
    }

    pub fn with_token(config: RemoteConfig, token: &str) -> Result<Self> {
        let mut client = Self::new(config, None)?;
        let mut header = HeaderValue::from_str(&format!("Bearer {token}"))?;
        header.set_sensitive(true);
        client.token = Some(header);
        Ok(client)
    }

    pub fn configured(name: &str, config: RemoteConfig) -> Result<Self> {
        match std::fs::read(token_path(name)?) {
            Ok(bytes) => {
                let token: crate::api::client_auth::Token =
                    serde_json::from_slice(&bytes).context("Invalid saved client token")?;
                return Self::with_token(config, &token.access_token);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let path = credential_path(name)?;
        let cookie = match std::fs::read_to_string(path) {
            Ok(cookie) => Some(cookie),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        Self::new(config, cookie.as_deref())
    }

    fn request<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let client = self.clone();
        let url = format!("{}{path}", client.config.url.trim_end_matches('/'));
        let bytes = std::thread::spawn(move || -> Result<Vec<u8>> {
            let http = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none()).build()?;
            let mut request = if let Some(body) = body { http.post(url).json(&body) } else { http.get(url) };
            if let Some(cookie) = client.cookie { request = request.header(COOKIE, cookie); }
            if let Some(token) = client.token { request = request.header(reqwest::header::AUTHORIZATION, token); }
            if let Some(user) = client.config.user_id { request = request.header("x-totui-expected-user", user); }
            let response = request.send().context("Remote request failed; local storage was not used")?;
            let status = response.status();
            if matches!(status.as_u16(), 401 | 403) || status.is_redirection() { bail!("Remote login required or expired. Run totui remote login NAME"); }
            if status.as_u16() == 404 { bail!("Remote API is unavailable. Upgrade the server to a version supporting remote clients"); }
            if status.as_u16() == 409 { bail!("Conflict: remote list or account changed. Reload before saving"); }
            if status.as_u16() == 412 { return Err(PreconditionFailed.into()); }
            ensure!(status.is_success(), "Remote server returned HTTP {status}");
            Ok(response.bytes()?.to_vec())
        }).join().map_err(|_| anyhow::anyhow!("Remote request worker failed"))??;
        serde_json::from_slice(&bytes)
            .context("Invalid remote response; check server version and proxy configuration")
    }

    pub fn revoke(&self) -> Result<()> {
        if self.token.is_some() {
            self.request::<serde_json::Value>("/api/remote/logout", Some(serde_json::json!({})))?;
        }
        Ok(())
    }

    pub fn user(&self) -> Result<User> {
        self.request(
            if self.token.is_some() {
                "/api/remote/me"
            } else {
                "/api/me"
            },
            None,
        )
    }

    pub fn call<T: DeserializeOwned>(&self, request: Request) -> Result<T> {
        if let Some(cache) = &self.cache {
            if let Some(value) = cache.call(&request)? {
                return Ok(serde_json::from_value(value)?);
            }
            ensure!(
                cache.status().pending == 0 && cache.status().conflicts == 0,
                "Finish syncing pending edits before project management or rollover"
            );
        }
        let result = self.request("/api/remote/v1", Some(serde_json::to_value(request)?))?;
        if let Some(cache) = &self.cache {
            cache.refresh_changes(self)?;
        }
        Ok(result)
    }

    pub fn cached(&self) -> Option<Arc<cache::Cache>> {
        self.cache.clone()
    }

    pub fn with_cache(mut self, root: &std::path::Path) -> Result<Self> {
        let cache = cache::Cache::open(root, || self.sync_snapshot())?;
        cache.start(self.clone());
        self.cache = Some(cache);
        Ok(self)
    }

    pub fn sync_changes(&self, cursor: &protocol::SyncCursor) -> Result<protocol::SyncChanges> {
        self.request(&format!("/api/remote/changes?since={cursor}"), None)
    }

    pub fn sync_apply_changes(
        &self,
        batch: &protocol::SyncBatch,
        cursor: &protocol::SyncCursor,
    ) -> Result<protocol::SyncChanges> {
        self.request(
            &format!("/api/remote/sync?since={cursor}"),
            Some(serde_json::to_value(batch)?),
        )
    }

    pub fn event_stream(
        &self,
        cursor: &protocol::SyncCursor,
        receive: impl FnMut(protocol::SyncChanges) -> Result<()>,
    ) -> Result<()> {
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(75))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut request = http
            .get(format!("{}/api/remote/events", self.config.url))
            .header("Last-Event-ID", cursor.to_string())
            .header("Accept", "text/event-stream");
        if let Some(cookie) = &self.cookie {
            request = request.header(COOKIE, cookie);
        }
        if let Some(token) = &self.token {
            request = request.header(reqwest::header::AUTHORIZATION, token);
        }
        if let Some(user) = &self.config.user_id {
            request = request.header("x-totui-expected-user", user);
        }
        let response = request.send()?;
        ensure!(
            response.status().is_success(),
            "Live sync returned HTTP {}; check your login or server connection",
            response.status()
        );
        ensure!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|s| s.starts_with("text/event-stream")),
            "Server did not return an event stream"
        );
        events::consume(std::io::BufReader::new(response), receive)
    }

    pub fn sync_snapshot(&self) -> Result<protocol::SyncSnapshot> {
        self.request("/api/remote/sync", None)
    }

    pub fn sync_apply(&self, batch: &protocol::SyncBatch) -> Result<protocol::SyncSnapshot> {
        self.request("/api/remote/sync", Some(serde_json::to_value(batch)?))
    }

    pub fn check(&self) -> Result<()> {
        let info: serde_json::Value = self.call(Request::Info)?;
        ensure!(info["protocol"] == 1, "Incompatible remote protocol");
        Ok(())
    }

    pub fn load(
        &self,
        project: &str,
        date: chrono::NaiveDate,
        history: bool,
        path: PathBuf,
    ) -> Result<TodoList> {
        if let Some(cache) = &self.cache {
            return cache.load(project, date, history, path);
        }
        let snapshot: Snapshot = self.call(Request::Load {
            project: project.into(),
            date,
            history,
        })?;
        Ok(snapshot.into_list(path))
    }

    pub fn save(&self, lists: &[(&TodoList, &str)]) -> Result<()> {
        if let Some(cache) = &self.cache {
            return cache.save(lists);
        }
        let revisions: Vec<i64> = self.call(Request::Save {
            lists: lists
                .iter()
                .map(|(list, project)| (project.to_string(), Snapshot::from(*list)))
                .collect(),
        })?;
        ensure!(
            revisions.len() == lists.len(),
            "Invalid remote save response; reload before retrying"
        );
        for ((list, _), revision) in lists.iter().zip(revisions) {
            list.revision.set(revision);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct PreconditionFailed;
impl std::fmt::Display for PreconditionFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("412 Precondition Failed: task changed")
    }
}
impl std::error::Error for PreconditionFailed {}
