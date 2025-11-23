use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub code_b64: String,
    pub priority: TaskPriority,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    High,
    Medium,
    Low,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskResult {
    pub id: Uuid,
    pub name: String,
    pub code_b64: String,
    pub result: Option<serde_json::Value>,
    pub status: TaskStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
}
