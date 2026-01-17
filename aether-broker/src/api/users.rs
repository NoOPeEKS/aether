use std::sync::Arc;

use aether_core::auth::{Permission, User};
use aether_core::http::CreateUserRequest;
use aether_core::traits::Storage;
use axum::Extension;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use bcrypt::{DEFAULT_COST, hash};
use uuid::Uuid;

use crate::state::BrokerState;

pub async fn create_user_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Extension(user): Extension<User>,
    Json(user_info): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if !user.permissions.contains(&Permission::CreateUser)
        && !user.permissions.contains(&Permission::All)
        && !user.is_admin
    {
        return StatusCode::UNAUTHORIZED;
    }
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
