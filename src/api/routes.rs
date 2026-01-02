use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use super::handlers;

async fn health_check() -> &'static str {
    "ok"
}

pub fn create_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/todos", get(handlers::list_todos))
        .route("/api/todos", post(handlers::create_todo))
        .route("/api/todos/{id}", delete(handlers::delete_todo))
        .route("/api/todos/{id}", patch(handlers::update_todo))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}
