use std::sync::Arc;

use aether_broker::{BrokerState, build_router};
use aether_core::auth::{Permission, User};
use aether_core::broker::storage::InMemoryStorage;
use aether_core::http::{CreateTaskRequest, CreateTaskResponse, GetAllTasksResponse};
use aether_core::task::{TaskPriority, TaskStatus};
use aether_core::traits::Storage;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use tower::Service;
use uuid::Uuid;

async fn get_test_utils() -> (Router, Arc<BrokerState<InMemoryStorage>>, String) {
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let st_clone = Arc::clone(&state);
    state
        .storage
        .create_user(User {
            id: Uuid::new_v4(),
            name: "admin".into(),
            password_hash: hash("admin", DEFAULT_COST).expect("To be able to hash."),
            is_admin: true,
            permissions: vec![Permission::All],
        })
        .await
        .unwrap();
    let mut app = build_router(state);
    let body =
        Body::from(serde_json::to_vec(&json!({"username": "admin", "password": "admin"})).unwrap());
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .header("Content-Type", "application/json")
                .uri("/api/v1/auth/login")
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    (
        app,
        st_clone,
        json["jwt"]
            .to_string()
            .strip_prefix("\"")
            .unwrap()
            .strip_suffix("\"")
            .unwrap()
            .to_owned(),
    )
}

#[tokio::test]
async fn test_create_task() {
    let (mut app, state, jwt) = get_test_utils().await;
    let body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(state.storage.high_prio.read().await.len(), 1);
    assert_eq!(state.storage.high_prio.read().await[0].name, "task1".to_string());
}
