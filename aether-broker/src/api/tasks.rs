use std::sync::Arc;

use aether_core::http::{
    CreateTaskRequest, CreateTaskResponse, GetAllTasksResponse, GetTaskResponse,
};
use aether_core::task::{Task, TaskStatus};
use aether_core::traits::Storage;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use tracing::info;
use uuid::Uuid;

use crate::state::BrokerState;

pub async fn create_task_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Json(task): Json<CreateTaskRequest>,
) -> (StatusCode, Json<CreateTaskResponse>) {
    info!("[INFO] A task has been requested at the POST /tasks");
    let id = Uuid::new_v4();
    let new_task = Task {
        id,
        name: task.name,
        code_b64: task.code_b64,
        priority: task.priority,
        capabilities: task.capabilities,
    };

    if state.storage.enqueue_task(new_task).await.is_ok() {
        (
            StatusCode::CREATED,
            Json(CreateTaskResponse::Ok {
                task_id: id,
                status: TaskStatus::Queued,
            }),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CreateTaskResponse::Error {
                message: String::from("Could not create task successfully."),
            }),
        )
    }
}

pub async fn get_task_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Path(task_id): Path<Uuid>,
) -> (StatusCode, Json<GetTaskResponse>) {
    if let Some(task) = state.storage.get_task_result(task_id).await {
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
            TaskStatus::Cancelled => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: Some("This task got cancelled due too many attempts".to_string()),
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

pub async fn get_all_tasks_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
) -> (StatusCode, Json<GetAllTasksResponse>) {
    if let Some(tasks) = state.storage.get_all_tasks().await {
        (
            StatusCode::OK,
            Json(GetAllTasksResponse { tasks: Some(tasks) }),
        )
    } else {
        (StatusCode::OK, Json(GetAllTasksResponse { tasks: None }))
    }
}
