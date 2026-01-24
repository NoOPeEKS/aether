use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capabilities::TaskCapabilities;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub code_b64: String,
    pub priority: TaskPriority,
    pub capabilities: Option<TaskCapabilities>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    High,
    Medium,
    Low,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TaskResult {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub code_b64: String,
    pub result: Option<serde_json::Value>,
    pub status: TaskStatus,
    pub capabilities: Option<TaskCapabilities>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Lease {
    pub worker_id: String,
    pub attempts: usize,
    pub start_time: SystemTime,
}
