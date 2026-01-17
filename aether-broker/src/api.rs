pub mod health;
pub mod tasks;
pub mod auth;
pub mod users;

use std::sync::Arc;

use aether_core::traits::Storage;
use axum::Router;
use axum::routing::{get, post};
use health::health_handler;
use tasks::{cancel_task_handler, create_task_handler, get_all_tasks_handler, get_task_handler};
use auth::login_handler;
use users::create_user_handler;

use crate::state::BrokerState;

pub fn build_router<S: Storage>(state: Arc<BrokerState<S>>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route(
            "/api/v1/tasks",
            post(create_task_handler).get(get_all_tasks_handler),
        )
        .route("/api/v1/tasks/{task_id}", get(get_task_handler))
        .route("/api/v1/tasks/{task_id}/cancel", post(cancel_task_handler))
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/users", post(create_user_handler))
        .with_state(state)
}
