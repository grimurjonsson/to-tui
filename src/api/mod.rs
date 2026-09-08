pub mod auth;
pub mod handlers;
pub mod models;
mod projects;
pub mod routes;

pub use routes::create_authenticated_router;
pub use routes::create_router;
pub mod web;
