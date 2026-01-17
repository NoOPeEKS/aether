use crate::state::BrokerState;
use aether_core::auth::{JWTClaims, User};
use aether_core::http::{LoginRequest, LoginResponse};
use aether_core::traits::Storage;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use bcrypt::verify;
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header};
use std::sync::Arc;

fn check_user_login(user: &User, password: &str) -> bool {
    verify(password, &user.password_hash).unwrap_or(false)
}

fn issue_jwt(user: &User, secret: &[u8]) -> anyhow::Result<String> {
    let now = Utc::now();
    let expires_at = now + Duration::days(30);

    let claims = JWTClaims {
        sub: user.id.to_string(),
        exp: expires_at.timestamp() as usize,
        iat: now.timestamp() as usize,
        iss: "aether-broker".into(),
        user_id: user.id,
        username: user.name.clone(),
        is_admin: user.is_admin,
        permissions: user.permissions.clone(),
    };

    Ok(jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )?)
}

pub async fn login_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Json(login_info): Json<LoginRequest>,
) -> (StatusCode, Json<LoginResponse>) {
    match state.storage.get_user(&login_info.username).await {
        Ok(Some(user)) => {
            if !check_user_login(&user, &login_info.password) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(LoginResponse::Err {
                        message: "Invalid login credentials.".into(),
                    }),
                );
            }
            match issue_jwt(&user, "placeholder-secret".as_bytes()) {
                Ok(jwt) => (StatusCode::OK, Json(LoginResponse::Ok { jwt })),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(LoginResponse::Err {
                        message: "Failed to generate token".into(),
                    }),
                ),
            }
        }
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse::Err {
                message: "Invalid login credentials.".into(),
            }),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LoginResponse::Err {
                message: "An error has occured.".into(),
            }),
        ),
    }
}
