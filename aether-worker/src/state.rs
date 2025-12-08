use std::collections::{HashMap, VecDeque};

use aether_common::task::Task;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub struct WorkerState {
    pub id: String,
    pub task_list: RwLock<VecDeque<Task>>,
    pub running_tasks: RwLock<HashMap<Uuid, RunningTask>>,
}

impl WorkerState {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            task_list: RwLock::new(VecDeque::new()),
            running_tasks: RwLock::new(HashMap::new()),
        }
    }
}

pub struct RunningTask {
    pub handle: JoinHandle<()>,
    pub cancel_tx: oneshot::Sender<()>,
}

#[derive(Deserialize, Serialize)]
pub struct PythonExecution {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
