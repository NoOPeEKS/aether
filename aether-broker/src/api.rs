mod state;
mod tasks;
mod health;
pub mod types;

use axum::Router;
use axum::routing::{get, post};

use tasks::submit_task_handler;
use health::health_handler;

pub use state::AppState;

pub fn build_router(state: state::AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/tasks", post(submit_task_handler))
        // .route("/api/v1/tasks/{task_id}", get(get_task_status_handler))
        .with_state(state)
}
