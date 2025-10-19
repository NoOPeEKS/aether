use aether_broker::{AppState, build_router};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::Service;

#[tokio::test]
async fn test_health_endpoint() {
    let state = AppState {
        queue: Arc::new(RwLock::new(HashMap::new())),
        results: Arc::new(RwLock::new(HashMap::new())),
    };
    let mut app = build_router(state);

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_string = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_string, r#"{"status":"healthy"}"#);
}
