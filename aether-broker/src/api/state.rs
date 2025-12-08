use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::json;

use crate::BrokerState;

pub async fn state_handler(
    State(state): State<Arc<BrokerState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let state_str = format!("{state:?}");
    (StatusCode::OK, Json(json!({"state": state_str})))
}
