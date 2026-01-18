use std::sync::Arc;

use aether_core::traits::Storage;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;

use crate::BrokerState;
use crate::api::auth::verify_jwt;

pub async fn auth_jwt_middleware<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims =
        verify_jwt(token, "placeholder-secret".as_bytes()).map_err(|_| StatusCode::UNAUTHORIZED)?;

    if Utc::now().timestamp() as usize > claims.exp {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user = state
        .storage
        .get_user(&claims.username)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}
