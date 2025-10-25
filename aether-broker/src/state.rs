use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{Mutex, RwLock, mpsc},
    time::Instant,
};
use uuid::Uuid;

use aether_common::task::{Task, TaskResult, TaskStatus};

#[derive(Clone, Debug)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub last_heartbeat: Instant,
    pub active: bool,
}

#[derive(Clone)]
pub struct BrokerState {
    pub queue_tx: mpsc::Sender<Task>,
    pub queue_rx: Arc<Mutex<mpsc::Receiver<Task>>>,
    pub tasks: Arc<RwLock<HashMap<Uuid, TaskResult>>>,
    pub worker_registry: Arc<RwLock<HashMap<String, WorkerInfo>>>,
}

impl BrokerState {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Task>(buffer_size);
        Self {
            queue_tx: tx,
            queue_rx: Arc::new(Mutex::new(rx)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            worker_registry: Arc::new(RwLock::new(HashMap::new())),
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

    pub async fn get_all_tasks(&self) -> Option<Vec<TaskResult>> {
        let tasks: Vec<TaskResult> = self.tasks.read().await.values().cloned().collect();
        if tasks.is_empty() { None } else { Some(tasks) }
    }
}
