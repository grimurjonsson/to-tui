use super::{handlers, web};
use axum::{
    Router,
    routing::{delete, get, post},
};
use tower_http::trace::TraceLayer;

async fn health_check() -> &'static str {
    "ok"
}

pub fn create_router() -> anyhow::Result<Router> {
    let state = web::observer()?;
    Ok(Router::new()
        .route("/", get(web::index))
        .route("/app.js", get(web::script))
        .route("/style.css", get(web::style))
        .route("/api/events", get(web::events))
        .route("/api/snapshot", get(web::snapshot))
        .route("/api/health", get(health_check))
        .route("/api/projects", get(handlers::list_projects))
        .route(
            "/api/todos",
            get(handlers::list_todos).post(handlers::create_todo),
        )
        .route(
            "/api/todos/{id}",
            delete(handlers::delete_todo).patch(handlers::update_todo),
        )
        .route("/api/todos/{id}/move", post(web::move_todo))
        .with_state(state)
        .layer(axum::middleware::from_fn(web::protect_local_writes))
        .layer(TraceLayer::new_for_http()))
}
