use aether_broker::{AppState, build_router};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::Service;

#[tokio::test]
async fn test_create_task() {
    let state = AppState {
        queue: Arc::new(RwLock::new(HashMap::new())),
        results: Arc::new(RwLock::new(HashMap::new())),
    };
    let mut app = build_router(state);

    let payload = json!({"name": "task1", "args": [1, 2, "hello"]});
    let body = Body::from(serde_json::to_vec(&payload).unwrap());
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_string = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_string.contains("queued"));
}
