use super::{auth, handlers, projects, web};
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
        .route("/app.js", get(web::script))
        .route("/style.css", get(web::style))
        .route("/favicon.ico", get(web::favicon))
        .route("/api/events", get(web::events))
        .route("/api/snapshot", get(web::snapshot))
        .route("/api/me", get(auth::me))
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
