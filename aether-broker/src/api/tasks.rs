use axum::extract::Path;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::api::state::AppState;
use crate::api::types::{Task, TaskStatus};

#[derive(Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct CreateTaskResponse {
    pub task_id: Uuid,
    pub status: TaskStatus,
}

pub async fn create_task_handler(
    State(state): State<AppState>,
    Json(task): Json<CreateTaskRequest>,
) -> (StatusCode, Json<CreateTaskResponse>) {
    let id = Uuid::new_v4();

    let new_task = Task {
        id,
        name: task.name,
        args: task.args,
        status: TaskStatus::Queued,
    };

    state.queue.write().await.insert(id, new_task);

    (
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            task_id: id,
            status: TaskStatus::Queued,
        }),
    )
}

#[derive(Serialize)]
pub struct GetTaskResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<Uuid>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<TaskStatus>,

    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> (StatusCode, Json<GetTaskResponse>) {
    if let Some(task) = state.queue.read().await.get(&task_id) {
        match task.status {
            TaskStatus::Completed => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task_id: Some(task.id),
                    name: Some(task.name.clone()),
                    status: Some(task.status.clone()),
                    args: Some(task.args.clone()),
                    result: Some(json!({"result": "dummy_value"})),
                    error: None,
                }),
            ),
            TaskStatus::Queued => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task_id: Some(task.id),
                    name: Some(task.name.clone()),
                    status: Some(task.status.clone()),
                    args: Some(task.args.clone()),
                    result: None,
                    error: None,
                }),
            ),

            TaskStatus::Running => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task_id: Some(task.id),
                    name: Some(task.name.clone()),
                    status: Some(task.status.clone()),
                    args: Some(task.args.clone()),
                    result: None,
                    error: None,
                }),
            ),
            TaskStatus::Failed => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task_id: Some(task.id),
                    name: Some(task.name.clone()),
                    status: Some(task.status.clone()),
                    args: Some(task.args.clone()),
                    result: None,
                    error: Some("An error occured parsing inputs.".to_string()),
                }),
            ),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(GetTaskResponse {
                task_id: None,
                name: None,
                status: None,
                args: None,
                result: None,
                error: Some("No task was found with the provided id".to_string()),
            }),
        )
    }
}
