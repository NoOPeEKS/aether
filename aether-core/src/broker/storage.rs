use crate::task::{Task, TaskPriority, TaskResult, TaskStatus};
use crate::traits::Lease;
use crate::traits::Storage;
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;
use tokio::time::Instant;
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct InMemoryStorageState {
    pub high_prio: RwLock<VecDeque<Task>>,
    pub mid_prio: RwLock<VecDeque<Task>>,
    pub low_prio: RwLock<VecDeque<Task>>,
    pub results: RwLock<HashMap<Uuid, TaskResult>>,
    pub leases: RwLock<HashMap<Uuid, Lease>>,
}

#[derive(Default)]
pub struct InMemoryStorage {
    pub state: InMemoryStorageState,
}

#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    async fn enqueue_task(&self, task: Task) {
        match task.priority {
            TaskPriority::High => self.state.high_prio.write().await.push_back(task),
            TaskPriority::Medium => self.state.mid_prio.write().await.push_back(task),
            TaskPriority::Low => self.state.low_prio.write().await.push_back(task),
        }
    }

    async fn dequeue_task(&self, worker_id: &str) -> Option<Task> {
        // pop from high, then mid, then low
        if let Some(t) = self.state.high_prio.write().await.pop_front() {
            let id = t.id;
            self.state.results.write().await.insert(
                id,
                TaskResult {
                    id,
                    name: t.name.clone(),
                    code_b64: t.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );

            let lease = Lease {
                worker_id: worker_id.to_owned(),
                attempts: 0,
                start_time: Instant::now(),
            };
            self.state.leases.write().await.insert(id, lease);
            return Some(t);
        }
        if let Some(t) = self.state.mid_prio.write().await.pop_front() {
            let id = t.id;
            self.state.results.write().await.insert(
                id,
                TaskResult {
                    id,
                    name: t.name.clone(),
                    code_b64: t.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );

            let lease = Lease {
                worker_id: worker_id.to_owned(),
                attempts: 0,
                start_time: Instant::now(),
            };
            self.state.leases.write().await.insert(id, lease);
            return Some(t);
        }
        if let Some(t) = self.state.low_prio.write().await.pop_front() {
            let id = t.id;
            self.state.results.write().await.insert(
                id,
                TaskResult {
                    id,
                    name: t.name.clone(),
                    code_b64: t.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );

            let lease = Lease {
                worker_id: worker_id.to_owned(),
                attempts: 0,
                start_time: Instant::now(),
            };
            self.state.leases.write().await.insert(id, lease);
            return Some(t);
        }
        None
    }

    async fn remove_lease(&self, task_id: &Uuid) -> Option<Lease> {
        self.state.leases.write().await.remove(task_id)
    }

    async fn store_result(&self, task_id: Uuid, result: TaskResult) {
        self.state.results.write().await.insert(task_id, result);
    }

    async fn get_result(&self, task_id: Uuid) -> Option<TaskResult> {
        self.state.results.read().await.get(&task_id).cloned()
    }
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            state: InMemoryStorageState::default(),
        }
    }
}
