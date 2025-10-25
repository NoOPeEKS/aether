use aether_broker::api::tasks::{CreateTaskResponse, GetAllTasksResponse};
use aether_broker::{BrokerState, build_router};
use aether_common::task::TaskStatus;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::Service;

#[tokio::test]
async fn test_create_task() {
    let state = BrokerState::new(10);
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

#[tokio::test]
async fn test_get_non_existant_task() {
    let state = BrokerState::new(10);
    let mut app = build_router(state);

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri("/api/v1/tasks/e6f8019a-ab04-46a1-b894-3c10c29e9d20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_string = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_string.contains("No task was found with the provided id"));
}

#[tokio::test]
async fn test_get_info_from_task() {
    let state = BrokerState::new(10);
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

    let create_resp_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_body_string = String::from_utf8(create_resp_body.to_vec()).unwrap();
    let create_task_resp: CreateTaskResponse = serde_json::from_str(&create_body_string).unwrap();

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", create_task_resp.task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let get_resp_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let get_body_string = String::from_utf8(get_resp_body.to_vec()).unwrap();

    assert!(get_body_string.contains(r#""name":"task1""#));
    assert!(get_body_string.contains(r#""status":"queued""#));
}

#[tokio::test]
async fn test_get_all_tasks() {
    let state = BrokerState::new(10);
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

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri("/api/v1/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let get_resp_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let get_body_string = String::from_utf8(get_resp_body.to_vec()).unwrap();
    let resp: GetAllTasksResponse = serde_json::from_str(&get_body_string).unwrap();
    assert!(resp.tasks.is_some());
    assert!(!resp.tasks.clone().unwrap().is_empty());
    assert_eq!(resp.tasks.unwrap()[0].status, TaskStatus::Queued);
}
