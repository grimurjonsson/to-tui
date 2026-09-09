use super::{auth, client_auth, handlers, projects, server_info, web};
use axum::{
    Router,
    routing::{delete, get, post},
};
use tower_http::trace::TraceLayer;

async fn health_check() -> &'static str {
    "ok"
}

async fn readiness_check(
    axum::Extension(state): axum::Extension<auth::ServerState>,
) -> axum::http::StatusCode {
    if state.ready().await {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    }
}

pub fn create_router() -> anyhow::Result<Router> {
    Ok(router(auth::ServerState::local()?))
}

pub fn create_authenticated_router(auth_url: &str) -> anyhow::Result<Router> {
    Ok(router(auth::ServerState::authenticated(auth_url)?))
}

fn router(state: auth::ServerState) -> Router {
    Router::new()
        .route("/", get(web::index))
        .route("/kanban", get(super::kanban::page))
        .route("/kanban.js", get(super::kanban::script))
        .route("/kanban.css", get(super::kanban::style))
        .route("/signed-out", get(server_info::signed_out))
        .route("/app.js", get(web::script))
        .route("/style.css", get(web::style))
        .route("/favicon.ico", get(web::favicon))
        .route("/fonts/IBMPlexSans-latin.woff2", get(web::font_sans))
        .route("/fonts/IBMPlexMono-400-latin.woff2", get(web::font_mono))
        .route(
            "/fonts/IBMPlexMono-500-latin.woff2",
            get(web::font_mono_medium),
        )
        .route("/api/events", get(web::events))
        .route(
            "/api/kanban",
            get(super::kanban::view).post(super::kanban::mutate),
        )
        .route("/api/snapshot", get(web::snapshot))
        .route("/api/me", get(auth::me))
        .route("/api/server", get(server_info::info))
        .route("/api/server/upgrade", post(server_info::upgrade))
        .route("/api/remote/v1", post(crate::remote::handle))
        .route(
            "/api/remote/sync",
            get(crate::remote::sync_api::snapshot).post(crate::remote::sync_api::apply),
        )
        .route(
            "/api/remote/items/{id}",
            get(crate::remote::sync_api::get_item)
                .put(crate::remote::sync_api::put_item)
                .delete(crate::remote::sync_api::delete_item),
        )
        .route("/api/remote/events", get(crate::remote::events::events))
        .route("/api/remote/changes", get(crate::remote::events::changes))
        .route("/api/remote/me", get(auth::me))
        .route("/api/remote/login/exchange", post(client_auth::exchange))
        .route("/api/remote/logout", post(client_auth::revoke))
        .route(
            "/remote/login",
            get(client_auth::login).post(client_auth::approve),
        )
        .route("/api/health", get(health_check))
        .route("/api/ready", get(readiness_check))
        .route(
            "/api/projects",
            get(handlers::list_projects).post(projects::create),
        )
        .route(
            "/api/projects/{id}",
            axum::routing::patch(projects::rename).delete(projects::delete),
        )
        .route(
            "/api/todos",
            get(handlers::list_todos).post(handlers::create_todo),
        )
        .route(
            "/api/todos/{id}",
            delete(handlers::delete_todo).patch(handlers::update_todo),
        )
        .route("/api/todos/{id}/move", post(web::move_todo))
        .layer(axum::middleware::from_fn(web::protect_local_writes))
        .layer(axum::Extension(state.clone()))
        .layer(axum::middleware::from_fn_with_state(state, auth::identify))
        .layer(TraceLayer::new_for_http())
}
