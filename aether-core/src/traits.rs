use crate::task::{Task, TaskResult};
use tokio::time::Instant;
use uuid::Uuid;

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

    /// Store a task result (completed/failed).
    async fn store_result(&self, task_id: Uuid, result: TaskResult);

    /// Get task result if present.
    async fn get_result(&self, task_id: Uuid) -> Option<TaskResult>;
}
