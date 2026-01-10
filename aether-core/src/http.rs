use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capabilities::TaskCapabilities;
use crate::task::{TaskPriority, TaskResult, TaskStatus};

#[derive(Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub code_b64: String,
    pub priority: TaskPriority,
    pub capabilities: Option<TaskCapabilities>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateTaskResponse {
    pub task_id: Uuid,
    pub status: TaskStatus,
}

#[derive(Serialize, Deserialize)]
pub struct GetTaskResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskResult>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetAllTasksResponse {
    pub tasks: Option<Vec<TaskResult>>,
}
