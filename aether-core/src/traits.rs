use std::collections::HashMap;

use tokio::time::Instant;
use uuid::Uuid;

use crate::task::{Task, TaskResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lease {
    pub worker_id: String,
    pub attempts: usize,
    pub start_time: Instant,
}

#[async_trait::async_trait]
pub trait Storage: Send + Sync + 'static {
    /// Insert a new task into the appropriate priority queue.
    async fn enqueue_task(&self, task: Task);

    /// Pop a task from queues in priority order and create a lease for worker_id.
    /// Returns the task id if a task was leased.
    async fn dequeue_task(&self, worker_id: &str) -> Option<Task>;

    /// Remove the lease for task id (worker finished or lost).
    async fn remove_lease(&self, task_id: &Uuid) -> Option<Lease>;

    /// Remove the lease for task id (worker finished or lost).
    async fn get_all_leases(&self) -> HashMap<Uuid, Lease>;

    /// Store a task result (completed/failed).
    async fn store_result(&self, task_id: Uuid, result: TaskResult);

    /// Checks whether the storage contains a result for the given task id.
    async fn contains_result(&self, task_id: Uuid) -> bool;

    /// Get task result if present.
    async fn get_task(&self, task_id: Uuid) -> Option<TaskResult>;

    /// Gets all tasks result if present.
    async fn get_all_tasks(&self) -> Option<Vec<TaskResult>>;

    async fn mark_task_failed(
        &self,
        task_id: &Uuid,
        max_attempts: usize,
    ) -> anyhow::Result<(bool, String)>;
}
