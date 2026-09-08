use super::protocol::{SyncChanges, SyncCursor};
use crate::storage::{change_log, context};
use anyhow::{Context, Result};
use axum::{
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use futures_util::{StreamExt, stream};
use serde::Deserialize;
use std::{
    collections::HashMap,
    convert::Infallible,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};

struct Hub {
    receiver: tokio::sync::watch::Receiver<u64>,
}
static HUBS: OnceLock<Mutex<HashMap<PathBuf, Weak<Hub>>>> = OnceLock::new();

fn hub(root: &PathBuf) -> Result<Arc<Hub>> {
    let mut hubs = HUBS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| anyhow::anyhow!("Observer registry lock failed"))?;
    hubs.retain(|_, h| h.strong_count() > 0);
    if let Some(hub) = hubs.get(root).and_then(Weak::upgrade) {
        return Ok(hub);
    }
    let hub = Arc::new(Hub {
        receiver: crate::api::web::observer()?.0,
    });
    hubs.insert(root.clone(), Arc::downgrade(&hub));
    Ok(hub)
}

#[derive(Debug, Default, Deserialize)]
pub struct Since {
    pub since: Option<String>,
}

pub async fn changes(Query(query): Query<Since>) -> Response {
    let Some(cursor) = query.since.and_then(|s| s.parse::<SyncCursor>().ok()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match context::blocking(move || change_log::changes(cursor)).await {
        Ok(Ok(changes)) => axum::Json(changes).into_response(),
        Ok(Err(error)) | Err(error) => crate::api::models::ErrorResponse::internal(error),
    }
}

pub async fn events(headers: HeaderMap) -> Response {
    let Some(cursor) = headers
        .get("last-event-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<SyncCursor>().ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let root = match crate::utils::paths::get_to_tui_dir() {
        Ok(root) => root,
        Err(error) => return crate::api::models::ErrorResponse::internal(error),
    };
    let root_copy = root.clone();
    let hub = match context::blocking(move || hub(&root_copy)).await {
        Ok(Ok(hub)) => hub,
        Ok(Err(error)) | Err(error) => return crate::api::models::ErrorResponse::internal(error),
    };
    let receiver = hub.receiver.clone();
    let stream = stream::unfold(
        (hub, receiver, root, Some(cursor), true),
        |(hub, mut receiver, root, cursor, first)| async move {
            let cursor = cursor?;
            if !first && receiver.changed().await.is_err() {
                return None;
            }
            receiver.borrow_and_update();
            let request_root = root.clone();
            let previous = cursor.clone();
            let result = tokio::task::spawn_blocking(move || {
                context::with_root(request_root, || change_log::changes(previous))
            })
            .await;
            let (event, cursor) = match result {
                Ok(Ok(mut changes)) => {
                    if serde_json::to_vec(&changes).ok()?.len() > 8 * 1024 * 1024 {
                        changes = SyncChanges::Reset;
                    }
                    let reset = matches!(changes, SyncChanges::Reset);
                    let next = match &changes {
                        SyncChanges::Delta(d) => d.cursor.clone(),
                        SyncChanges::Reset => cursor.clone(),
                    };
                    let event = Event::default()
                        .event("sync")
                        .id(next.to_string())
                        .json_data(changes)
                        .ok()?;
                    (event, if reset { None } else { Some(next) })
                }
                _ => return None,
            };
            Some((
                Ok::<_, Infallible>(event),
                (hub, receiver, root, cursor, false),
            ))
        },
    )
    .take_until(tokio::time::sleep(Duration::from_secs(60)));
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response();
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

pub(super) fn consume(
    reader: impl std::io::BufRead,
    mut receive: impl FnMut(SyncChanges) -> Result<()>,
) -> Result<()> {
    use std::io::{BufRead, Read};
    let mut reader = reader;
    let mut data = String::new();
    let mut event = String::new();
    loop {
        let mut line = String::new();
        let read = reader
            .by_ref()
            .take(16 * 1024 * 1024)
            .read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        anyhow::ensure!(line.ends_with('\n'), "Incomplete or oversized SSE line");
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if event == "sync" && !data.is_empty() {
                receive(serde_json::from_str(&data).context("Invalid sync event")?)?;
            }
            event.clear();
            data.clear();
        } else if let Some(value) = line.strip_prefix("event:") {
            event = value.trim_start().to_owned();
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.strip_prefix(' ').unwrap_or(value));
            anyhow::ensure!(data.len() < 16 * 1024 * 1024, "Oversized SSE event");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parser_handles_heartbeats_and_partial_frames() {
        let wire = b": heartbeat\r\n\r\nevent: sync\r\nid: ignored\r\ndata: {\r\ndata: \"kind\":\"reset\"}\r\n\r\nevent: sync\ndata: {\"kind\":\"reset\"}\n";
        let mut events = 0;
        consume(std::io::BufReader::with_capacity(2, &wire[..]), |change| {
            assert!(matches!(change, SyncChanges::Reset));
            events += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(events, 1);
    }

    #[test]
    fn test_observer_is_shared_within_workspace_and_separated_between_accounts() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let a = context::with_root(first.path().to_path_buf(), || {
            hub(&first.path().to_path_buf()).unwrap()
        });
        let b = context::with_root(first.path().to_path_buf(), || {
            hub(&first.path().to_path_buf()).unwrap()
        });
        let c = context::with_root(second.path().to_path_buf(), || {
            hub(&second.path().to_path_buf()).unwrap()
        });
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &c));
    }
}
