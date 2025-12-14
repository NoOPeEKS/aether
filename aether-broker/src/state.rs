use std::collections::{HashMap, VecDeque};

use aether_common::task::{Task, TaskPriority, TaskResult, TaskStatus};
use tokio::sync::RwLock;
use tokio::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub last_heartbeat: Instant,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct WorkerSession {
    pub sender: tokio::sync::mpsc::UnboundedSender<String>,
    pub connected_at: tokio::time::Instant,
}

/// Represents a lease of a task to a worker to control who has tasks
/// under execution and allow them to go back into the queue if finished.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lease {
    pub worker_id: String,
    pub attempts: usize,
    pub start_time: Instant,
}

#[derive(Default, Debug)]
pub struct BrokerState {
    pub high_prio: RwLock<VecDeque<Task>>,
    pub mid_prio: RwLock<VecDeque<Task>>,
    pub low_prio: RwLock<VecDeque<Task>>,
    pub results: RwLock<HashMap<Uuid, TaskResult>>,
    pub leases: RwLock<HashMap<Uuid, Lease>>,
    pub worker_registry: RwLock<HashMap<String, WorkerInfo>>,
    pub worker_sessions: RwLock<HashMap<String, WorkerSession>>,
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
            self.results.write().await.insert(
                task.id,
                TaskResult {
                    id: task.id,
                    name: task.name.clone(),
                    code_b64: task.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );
            self.leases.write().await.insert(
                task.id,
                Lease {
                    worker_id: worker_id.into(),
                    attempts: 1,
                    start_time: Instant::now(),
                },
            );
            Some(task)
        } else if let Some(task) = self.mid_prio.write().await.pop_front() {
            self.results.write().await.insert(
                task.id,
                TaskResult {
                    id: task.id,
                    name: task.name.clone(),
                    code_b64: task.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );
            self.leases.write().await.insert(
                task.id,
                Lease {
                    worker_id: worker_id.into(),
                    attempts: 1,
                    start_time: Instant::now(),
                },
            );
            Some(task)
        } else if let Some(task) = self.low_prio.write().await.pop_front() {
            self.results.write().await.insert(
                task.id,
                TaskResult {
                    id: task.id,
                    name: task.name.clone(),
                    code_b64: task.code_b64.clone(),
                    result: None,
                    status: TaskStatus::Running,
                },
            );
            self.leases.write().await.insert(
                task.id,
                Lease {
                    worker_id: worker_id.into(),
                    attempts: 1,
                    start_time: Instant::now(),
                },
            );
            Some(task)
        } else {
            None
        }
    }

    pub async fn update_result(&self, id: Uuid, result: serde_json::Value) {
        if let Some(t) = self.results.write().await.get_mut(&id) {
            t.status = TaskStatus::Completed;
            t.result = Some(result);
        }
    }

    pub async fn get_task(&self, id: Uuid) -> Option<TaskResult> {
        self.results.read().await.get(&id).cloned()
    }

    pub async fn get_all_tasks(&self) -> Option<Vec<TaskResult>> {
        let tasks: Vec<TaskResult> = self.results.read().await.values().cloned().collect();
        if tasks.is_empty() { None } else { Some(tasks) }
    }
}
