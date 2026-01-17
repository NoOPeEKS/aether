use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::Permission;
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
pub enum CreateTaskResponse {
    Ok { task_id: Uuid, status: TaskStatus },
    Error { message: String },
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

#[derive(Serialize, Deserialize)]
pub struct CancelTaskResponse {
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub enum LoginResponse {
    Ok { jwt: String },
    Err { message: String },
}

#[derive(Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
    pub permissions: Vec<Permission>,
}
