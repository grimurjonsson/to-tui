use super::auth::User;
use super::models::ErrorResponse;
use axum::{
    Extension, Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const REQUEST: &str = "/var/lib/totui/upgrade-request";
const STATUS: &str = "/var/lib/totui/upgrade-status.json";
const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Default)]
struct Release {
    latest_version: Option<String>,
    update_available: bool,
    check_error: Option<String>,
}

async fn release() -> Release {
    static CACHE: OnceLock<Mutex<Option<(Instant, Release)>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().await;
    if let Some((checked, release)) = &*cache
        && checked.elapsed() < Duration::from_secs(3600)
    {
        return release.clone();
    }
    let result = async {
        let value: serde_json::Value = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?
            .get("https://api.github.com/repos/grimurjonsson/to-tui/releases/latest")
            .header("User-Agent", "totui-server")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        anyhow::ensure!(
            value["draft"] == false && value["prerelease"] == false,
            "No stable release"
        );
        let version = value["tag_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing release version"))?
            .trim_start_matches('v');
        let latest = semver::Version::parse(version)?;
        Ok::<_, anyhow::Error>(Release {
            latest_version: Some(latest.to_string()),
            update_available: latest > semver::Version::parse(CURRENT)?,
            check_error: None,
        })
    }
    .await;
    let release = result.unwrap_or_else(|error| {
        tracing::warn!(%error, "Server update check failed");
        Release {
            check_error: Some("Could not check for updates".into()),
            ..Default::default()
        }
    });
    *cache = Some((Instant::now(), release.clone()));
    release
}

fn owner_matches(user: &User, owner: Option<&str>, enabled: bool) -> bool {
    enabled && user.id != "local" && owner.is_some_and(|id| !id.is_empty() && id == user.id)
}

fn can_upgrade(user: &User) -> bool {
    owner_matches(
        user,
        std::env::var("TOTUI_SERVER_OWNER_ID").ok().as_deref(),
        cfg!(target_os = "linux") && std::env::var("TOTUI_WEB_UPGRADE").as_deref() == Ok("1"),
    )
}

fn avatar_url(email: Option<&str>) -> Option<String> {
    let email = email?.trim().to_lowercase();
    if email.is_empty() {
        return None;
    }
    let hash = Sha256::digest(email.as_bytes());
    Some(format!(
        "https://gravatar.com/avatar/{hash:x}?s=80&d=404&r=g"
    ))
}

pub async fn info(Extension(user): Extension<User>) -> Response {
    let release = release().await;
    let status = std::fs::read(STATUS)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    Json(serde_json::json!({
        "version": CURRENT,
        "user": user,
        "avatar_url": avatar_url(user.email.as_deref()),
        "logout_url": if user.id == "local" { None } else { Some("/oauth2/sign_out?rd=%2Fsigned-out") },
        "can_upgrade": can_upgrade(&user),
        "latest_version": release.latest_version,
        "update_available": release.update_available,
        "check_error": release.check_error,
        "upgrade_pending": Path::new(REQUEST).exists(),
        "upgrade_status": status,
    }))
    .into_response()
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpgradeRequest {
    version: String,
}

pub async fn upgrade(
    Extension(user): Extension<User>,
    Json(request): Json<UpgradeRequest>,
) -> Response {
    if !can_upgrade(&user) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "Only the configured server owner can upgrade",
            )),
        )
            .into_response();
    }
    let latest = release().await;
    if !latest.update_available || latest.latest_version.as_deref() != Some(&request.version) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(
                "The available release changed. Refresh before upgrading.",
            )),
        )
            .into_response();
    }
    let temporary = format!("{REQUEST}.{}", uuid::Uuid::new_v4());
    let result = std::fs::write(&temporary, request.version.as_bytes())
        .and_then(|()| std::fs::hard_link(&temporary, REQUEST));
    let _ = std::fs::remove_file(&temporary);
    match result {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"status": "queued"})),
        )
            .into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (
            StatusCode::CONFLICT,
            Json(ErrorResponse::new("An upgrade is already running")),
        )
            .into_response(),
        Err(error) => ErrorResponse::internal(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_requires_enabled_explicit_nonlocal_owner() {
        let user = User {
            id: "owner-id".into(),
            email: Some("owner@example.com".into()),
        };
        assert!(owner_matches(&user, Some("owner-id"), true));
        assert!(!owner_matches(&user, Some("owner-id"), false));
        assert!(!owner_matches(&user, None, true));
        assert!(!owner_matches(&user, Some("someone-else"), true));
        assert!(!owner_matches(&user, Some("owner@example.com"), true));
        assert!(!owner_matches(
            &User {
                id: "local".into(),
                email: None
            },
            Some("local"),
            true
        ));
    }
}

pub async fn signed_out() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>totui · Signed out</title><main style="font:18px system-ui;max-width:36rem;margin:15vh auto;padding:2rem"><h1>You’re signed out</h1><p>Your server session has ended. Your Google account remains signed in.</p><a href="/">Sign in to totui</a></main></html>"#,
    )
}
