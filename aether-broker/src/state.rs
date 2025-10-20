use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, mpsc};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskResult {
    pub id: Uuid,
    pub name: String,
    pub args: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub status: TaskStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Clone)]
pub struct BrokerState {
    pub queue_tx: mpsc::Sender<Task>,
    pub queue_rx: Arc<Mutex<mpsc::Receiver<Task>>>,
    pub tasks: Arc<RwLock<HashMap<Uuid, TaskResult>>>,
}

impl BrokerState {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Task>(buffer_size);
        Self {
            queue_tx: tx,
            queue_rx: Arc::new(Mutex::new(rx)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn enqueue_task(&self, task: Task) {
        let task_send = task.clone();
        self.tasks.write().await.insert(
            task.id,
            TaskResult {
                id: task.id,
                name: task.name,
                args: task.args,
                result: None,
                status: TaskStatus::Queued,
            },
        );
        _ = self.queue_tx.send(task_send.clone());
    }

    pub async fn dequeue_task(&self) -> Option<Task> {
        let mut rx = self.queue_rx.lock().await;
        rx.recv().await
    }

    pub async fn update_result(&self, id: Uuid, result: serde_json::Value) {
        if let Some(t) = self.tasks.write().await.get_mut(&id) {
            t.status = TaskStatus::Completed;
            t.result = Some(result);
        }
    }

    pub async fn get_task(&self, id: Uuid) -> Option<TaskResult> {
        self.tasks.read().await.get(&id).cloned()
    }
}
