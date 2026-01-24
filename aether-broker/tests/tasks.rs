use std::sync::Arc;

use aether_broker::{BrokerState, build_router};
use aether_core::auth::{Permission, User};
use aether_core::broker::storage::InMemoryStorage;
use aether_core::http::{
    CancelTaskResponse, CreateTaskRequest, CreateTaskResponse, GetAllTasksResponse, GetTaskResponse,
};
use aether_core::task::{TaskPriority, TaskStatus};
use aether_core::traits::Storage;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use tower::Service;
use uuid::Uuid;

async fn get_admin_utils() -> (Router, Arc<BrokerState<InMemoryStorage>>, String, Uuid) {
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let st_clone = Arc::clone(&state);
    let user_id = Uuid::new_v4();
    state
        .storage
        .create_user(User {
            id: user_id,
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
        user_id,
    )
}

async fn get_user_utils_with_permissions(
    permissions: Vec<Permission>,
) -> (Router, Arc<BrokerState<InMemoryStorage>>, String, Uuid) {
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let st_clone = Arc::clone(&state);
    let user_id = Uuid::new_v4();
    state
        .storage
        .create_user(User {
            id: user_id,
            name: "user".into(),
            password_hash: hash("user", DEFAULT_COST).expect("To be able to hash."),
            is_admin: false,
            permissions,
        })
        .await
        .unwrap();
    let mut app = build_router(state);
    let body =
        Body::from(serde_json::to_vec(&json!({"username": "user", "password": "user"})).unwrap());
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
        user_id,
    )
}

async fn get_user_without_permissions() -> (Router, Arc<BrokerState<InMemoryStorage>>, String, Uuid)
{
    get_user_utils_with_permissions(vec![]).await
}

#[tokio::test]
async fn test_create_task_success() {
    let (mut app, state, jwt, _) = get_admin_utils().await;
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
    assert_eq!(
        state.storage.high_prio.read().await[0].name,
        "task1".to_string()
    );
}

#[tokio::test]
async fn test_create_task_unauthorized_no_permission() {
    let (mut app, _, jwt, _) = get_user_without_permissions().await;
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
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_task_success_with_permission() {
    let (mut app, state, jwt, _) =
        get_user_utils_with_permissions(vec![Permission::CreateTask]).await;
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
    assert_eq!(
        state.storage.high_prio.read().await[0].name,
        "task1".to_string()
    );
}

#[tokio::test]
async fn test_get_task_success() {
    let (mut app, state, jwt, user_id) = get_admin_utils().await;
    // First create a task
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let task_result = aether_core::task::TaskResult {
        id: task_id,
        owner_id: user_id,
        name: "task1".into(),
        code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
        result: None,
        status: TaskStatus::Queued,
        capabilities: None,
    };
    state.storage.store_result(task_id, task_result).await;

    let get_response = app
        .call(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body_bytes = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let get_json: GetTaskResponse = serde_json::from_slice(&get_body_bytes).unwrap();
    assert!(get_json.task.is_some());
    assert_eq!(get_json.task.as_ref().unwrap().name, "task1");
    assert!(get_json.error.is_none());
}

#[tokio::test]
async fn test_get_task_not_found() {
    let (mut app, _, jwt, _) = get_admin_utils().await;
    let task_id = Uuid::new_v4();
    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: GetTaskResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert!(json.task.is_none());
    assert!(json.error.is_some());
}

#[tokio::test]
async fn test_get_task_unauthorized() {
    let (mut app, state, jwt, user_id) = get_admin_utils().await;
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let task_result = aether_core::task::TaskResult {
        id: task_id,
        owner_id: user_id,
        name: "task1".into(),
        code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
        result: None,
        status: TaskStatus::Queued,
        capabilities: None,
    };
    state.storage.store_result(task_id, task_result).await;

    let (_, _, user_jwt, _) = get_user_without_permissions().await;
    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {user_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_task_with_permission() {
    let (mut app, state, admin_jwt, user_id) = get_admin_utils().await;
    let user_id_check = Uuid::new_v4();
    state
        .storage
        .create_user(User {
            id: user_id_check,
            name: "checkuser".into(),
            password_hash: hash("checkuser", DEFAULT_COST).expect("To be able to hash."),
            is_admin: false,
            permissions: vec![Permission::CheckTask],
        })
        .await
        .unwrap();
    // Login as checkuser
    let login_body = Body::from(
        serde_json::to_vec(&json!({"username": "checkuser", "password": "checkuser"})).unwrap(),
    );
    let login_response = app
        .call(
            Request::builder()
                .method("POST")
                .header("Content-Type", "application/json")
                .uri("/api/v1/auth/login")
                .body(login_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let login_body_bytes = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_json: serde_json::Value = serde_json::from_slice(&login_body_bytes).unwrap();
    let user_jwt = login_json["jwt"]
        .to_string()
        .strip_prefix("\"")
        .unwrap()
        .strip_suffix("\"")
        .unwrap()
        .to_owned();

    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {admin_jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let task_result = aether_core::task::TaskResult {
        id: task_id,
        owner_id: user_id,
        name: "task1".into(),
        code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
        result: None,
        status: TaskStatus::Queued,
        capabilities: None,
    };
    state.storage.store_result(task_id, task_result).await;

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("Authorization", format!("Bearer {user_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_all_tasks_success() {
    let (mut app, state, jwt, user_id) = get_admin_utils().await;
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let task_result = aether_core::task::TaskResult {
        id: task_id,
        owner_id: user_id,
        name: "task1".into(),
        code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
        result: None,
        status: TaskStatus::Queued,
        capabilities: None,
    };
    state.storage.store_result(task_id, task_result).await;

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri("/api/v1/tasks")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: GetAllTasksResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert!(json.tasks.is_some());
    assert_eq!(json.tasks.as_ref().unwrap().len(), 1);
}

#[tokio::test]
async fn test_get_all_tasks_unauthorized() {
    let (mut app, _, jwt, _) = get_admin_utils().await;
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let _ = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();

    let (_, _, user_jwt, _) = get_user_without_permissions().await;
    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri("/api/v1/tasks")
                .header("Authorization", format!("Bearer {user_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_all_tasks_with_permission() {
    let (mut app, state, admin_jwt, user_id) = get_admin_utils().await;
    let user_id_list = Uuid::new_v4();
    state
        .storage
        .create_user(User {
            id: user_id_list,
            name: "listuser".into(),
            password_hash: hash("listuser", DEFAULT_COST).expect("To be able to hash."),
            is_admin: false,
            permissions: vec![Permission::ListTasks],
        })
        .await
        .unwrap();
    let login_body = Body::from(
        serde_json::to_vec(&json!({"username": "listuser", "password": "listuser"})).unwrap(),
    );
    let login_response = app
        .call(
            Request::builder()
                .method("POST")
                .header("Content-Type", "application/json")
                .uri("/api/v1/auth/login")
                .body(login_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let login_body_bytes = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_json: serde_json::Value = serde_json::from_slice(&login_body_bytes).unwrap();
    let user_jwt = login_json["jwt"]
        .to_string()
        .strip_prefix("\"")
        .unwrap()
        .strip_suffix("\"")
        .unwrap()
        .to_owned();

    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {admin_jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let task_result = aether_core::task::TaskResult {
        id: task_id,
        owner_id: user_id,
        name: "task1".into(),
        code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
        result: None,
        status: TaskStatus::Queued,
        capabilities: None,
    };
    state.storage.store_result(task_id, task_result).await;

    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri("/api/v1/tasks")
                .header("Authorization", format!("Bearer {user_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_cancel_task_success() {
    let (mut app, state, jwt, user_id) =
        get_user_utils_with_permissions(vec![Permission::CreateTask, Permission::CancelTask]).await;
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let task_result = aether_core::task::TaskResult {
        id: task_id,
        owner_id: user_id,
        name: "task1".into(),
        code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
        result: None,
        status: TaskStatus::Queued,
        capabilities: None,
    };
    state.storage.store_result(task_id, task_result).await;

    let cancel_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/cancel", task_id))
                .header("Authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel_response.status(), StatusCode::OK);
    let cancel_body_bytes = axum::body::to_bytes(cancel_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let cancel_json: CancelTaskResponse = serde_json::from_slice(&cancel_body_bytes).unwrap();
    assert!(cancel_json.message.contains("cancelled successfully"));
}

#[tokio::test]
async fn test_cancel_task_not_found() {
    let (mut app, _, jwt, _) = get_admin_utils().await;
    let task_id = Uuid::new_v4();
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/cancel", task_id))
                .header("Authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_cancel_task_unauthorized_no_permission() {
    let (mut app, _, jwt, _) = get_user_utils_with_permissions(vec![Permission::CreateTask]).await;
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let (_, _, user_jwt, _) = get_user_without_permissions().await;
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/cancel", task_id))
                .header("Authorization", format!("Bearer {user_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_cancel_task_unauthorized_not_owner() {
    let (mut app, _, jwt, _) = get_admin_utils().await;
    let create_body = Body::from(
        serde_json::to_vec(&CreateTaskRequest {
            name: "task1".into(),
            code_b64: "cHJpbnQoImhlbGxvIHdvcmxkIik=".into(),
            priority: TaskPriority::High,
            capabilities: None,
        })
        .unwrap(),
    );
    let create_response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {jwt}"))
                .body(create_body)
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body_bytes = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: CreateTaskResponse = serde_json::from_slice(&create_body_bytes).unwrap();
    let task_id = match create_json {
        CreateTaskResponse::Ok { task_id, .. } => task_id,
        _ => panic!("Expected Ok response"),
    };

    let (_, _, user_jwt, _) = get_user_utils_with_permissions(vec![Permission::CancelTask]).await;
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/tasks/{}/cancel", task_id))
                .header("Authorization", format!("Bearer {user_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
