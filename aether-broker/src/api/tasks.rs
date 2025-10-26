use axum::extract::Path;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use aether_common::task::{Task, TaskResult, TaskStatus};

use crate::state::BrokerState;

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
    State(state): State<BrokerState>,
    Json(task): Json<CreateTaskRequest>,
) -> (StatusCode, Json<CreateTaskResponse>) {
    let id = Uuid::new_v4();
    let new_task = Task {
        id,
        name: task.name,
        args: task.args,
    };

    // TODO: Handle non able to enqueue correctly.
    state.enqueue_task(new_task).await;

    (
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            task_id: id,
            status: TaskStatus::Queued,
        }),
    )
}

#[derive(Serialize, Deserialize)]
pub struct GetTaskResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<TaskResult>,

    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub async fn get_task_handler(
    State(state): State<BrokerState>,
    Path(task_id): Path<Uuid>,
) -> (StatusCode, Json<GetTaskResponse>) {
    if let Some(task) = state.get_task(task_id).await {
        match task.status {
            TaskStatus::Completed => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: None,
                }),
            ),
            TaskStatus::Queued => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: None,
                }),
            ),

            TaskStatus::Running => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: None,
                }),
            ),
            TaskStatus::Failed => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: Some("An error occured parsing inputs.".to_string()),
                }),
            ),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(GetTaskResponse {
                task: None,
                error: Some("No task was found with the provided id".to_string()),
            }),
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct GetAllTasksResponse {
    pub tasks: Option<Vec<TaskResult>>,
}

pub async fn get_all_tasks_handler(
    State(state): State<BrokerState>,
) -> (StatusCode, Json<GetAllTasksResponse>) {
    if let Some(tasks) = state.get_all_tasks().await {
        (
            StatusCode::OK,
            Json(GetAllTasksResponse { tasks: Some(tasks) }),
        )
    } else {
        (StatusCode::OK, Json(GetAllTasksResponse { tasks: None }))
    }
}
