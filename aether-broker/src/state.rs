use std::collections::{HashMap, VecDeque};

use tokio::{sync::RwLock, time::Instant};
use uuid::Uuid;

use aether_common::task::{Task, TaskPriority, TaskResult, TaskStatus};

#[derive(Clone, Debug)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub last_heartbeat: Instant,
    pub active: bool,
}

pub struct Lease {}

#[derive(Default)]
pub struct BrokerState {
    pub high_prio: RwLock<VecDeque<Task>>,
    pub mid_prio: RwLock<VecDeque<Task>>,
    pub low_prio: RwLock<VecDeque<Task>>,
    pub leases: RwLock<HashMap<Uuid, TaskResult>>,
    pub worker_registry: RwLock<HashMap<String, WorkerInfo>>,
}

impl BrokerState {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub async fn enqueue_task(&self, task: Task) {
        match task.priority {
            TaskPriority::High => {
                self.high_prio.write().await.push_back(task);
            }
            TaskPriority::Medium => {
                self.mid_prio.write().await.push_back(task);
            }
            TaskPriority::Low => {
                self.low_prio.write().await.push_back(task);
            }
        };
    }

    pub async fn dequeue_task(&self, worker_id: &str) -> Option<Task> {
        if let Some(task) = self.high_prio.write().await.pop_front() {
            self.leases.write().await.insert(
                task.id,
                TaskResult {
                    id: task.id,
                    name: task.name.clone(),
                    code_b64: task.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );
            Some(task)
        } else if let Some(task) = self.mid_prio.write().await.pop_front() {
            self.leases.write().await.insert(
                task.id,
                TaskResult {
                    id: task.id,
                    name: task.name.clone(),
                    code_b64: task.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );
            Some(task)
        } else if let Some(task) = self.low_prio.write().await.pop_front() {
            self.leases.write().await.insert(
                task.id,
                TaskResult {
                    id: task.id,
                    name: task.name.clone(),
                    code_b64: task.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );
            Some(task)
        } else {
            None
        }
    }

    pub async fn update_result(&self, id: Uuid, result: serde_json::Value) {
        if let Some(t) = self.leases.write().await.get_mut(&id) {
            t.status = TaskStatus::Completed;
            t.result = Some(result);
        }
    }

    pub async fn get_task(&self, id: Uuid) -> Option<TaskResult> {
        self.leases.read().await.get(&id).cloned()
    }

    pub async fn get_all_tasks(&self) -> Option<Vec<TaskResult>> {
        let tasks: Vec<TaskResult> = self.leases.read().await.values().cloned().collect();
        if tasks.is_empty() { None } else { Some(tasks) }
    }
}
