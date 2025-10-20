pub mod health;
pub mod tasks;

use axum::Router;
use axum::routing::{get, post};

use crate::state::BrokerState;
use health::health_handler;
use tasks::{create_task_handler, get_task_handler};

pub fn build_router(state: BrokerState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/tasks", post(create_task_handler))
        .route("/api/v1/tasks/{task_id}", get(get_task_handler))
        .with_state(state)
}
