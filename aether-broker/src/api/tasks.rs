use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
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

pub async fn submit_task_handler(
    State(state): State<AppState>,
    Json(task): Json<CreateTaskRequest>,
) -> (StatusCode, Json<CreateTaskResponse>) {
    let id = Uuid::new_v4();

    let new_task = Task {
        id,
        name: task.name,
        args: task.args,
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
