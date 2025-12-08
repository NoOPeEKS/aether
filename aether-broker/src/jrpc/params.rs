use aether_common::task::Task;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct StopExecutionNotificationParams {
    pub task_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterWorkerRequestParams {
    pub worker_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterWorkerResponseParams {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FetchTaskRequestParams {
    pub worker_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FetchTaskResponseResult {
    pub task: Option<Task>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HeartbeatNotificationParams {
    pub worker_id: String,
}
