mod state;
pub mod types;
pub mod tasks;
pub mod health;

use axum::Router;
use axum::routing::{get, post};

use tasks::{create_task_handler, get_task_handler};
use health::health_handler;

pub use state::AppState;

pub fn build_router(state: state::AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/tasks", post(create_task_handler))
        .route("/api/v1/tasks/{task_id}", get(get_task_handler))
        .with_state(state)
}
