use crate::state::BrokerState;
use aether_core::http::{LoginRequest, LoginResponse};
use aether_core::traits::Storage;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use std::sync::Arc;

pub async fn login_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Json(login_info): Json<LoginRequest>,
) -> (StatusCode, Json<LoginResponse>) {
    (StatusCode::OK, Json(LoginResponse { jwt: "Placeholder".into() }))
}
