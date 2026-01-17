use crate::state::BrokerState;
use aether_core::auth::User;
use aether_core::http::CreateUserRequest;
use aether_core::traits::Storage;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use bcrypt::{DEFAULT_COST, hash};
use std::sync::Arc;
use uuid::Uuid;

pub async fn create_user_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Json(user_info): Json<CreateUserRequest>,
) -> StatusCode {
    // TODO: In the future, protect this endpoint by middleware so that only admins
    // can create new users.
    if let Ok(pass_hash) = hash(user_info.password, DEFAULT_COST) {
        let user = User {
            id: Uuid::new_v4(),
            name: user_info.username,
            password_hash: pass_hash,
            is_admin: user_info.is_admin,
            permissions: user_info.permissions,
        };
        match state.storage.create_user(user).await {
            Ok(_) => return StatusCode::CREATED,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    StatusCode::INTERNAL_SERVER_ERROR
}
