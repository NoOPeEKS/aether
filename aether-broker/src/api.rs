pub mod auth;
pub mod health;
pub mod middleware;
pub mod tasks;
pub mod users;

use std::sync::Arc;

use aether_core::traits::Storage;
use auth::login_handler;
use axum::Router;
use axum::routing::{get, post};
use health::health_handler;
use tasks::{cancel_task_handler, create_task_handler, get_all_tasks_handler, get_task_handler};
use users::create_user_handler;

use crate::api::middleware::auth_jwt_middleware;
use crate::state::BrokerState;

pub fn build_router<S: Storage>(state: Arc<BrokerState<S>>) -> Router {
    let public_routes = Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/auth/login", post(login_handler));

    let authed_routes = Router::new()
        .route(
            "/api/v1/tasks",
            post(create_task_handler).get(get_all_tasks_handler),
        )
        .route("/api/v1/tasks/{task_id}", get(get_task_handler))
        .route("/api/v1/tasks/{task_id}/cancel", post(cancel_task_handler))
        .route("/api/v1/users", post(create_user_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_jwt_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(authed_routes)
        .with_state(state)
}
