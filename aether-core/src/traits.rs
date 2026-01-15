use std::collections::HashMap;

use uuid::Uuid;

use crate::broker::storage::WorkerInfo;
use crate::capabilities::WorkerCapabilities;
use crate::task::{Lease, Task, TaskResult};

#[async_trait::async_trait]
pub trait Broker: Send + Sync {
    async fn run(&self, http_port: usize, rpc_port: usize) -> anyhow::Result<()>;
    async fn run_http_server(&self, port: usize) -> anyhow::Result<()>;
    async fn run_jrpc_server(&self, port: usize) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait Storage: Send + Sync + 'static {
    /// Insert a new task into the appropriate priority queue.
    async fn enqueue_task(&self, task: Task) -> anyhow::Result<()>;

    /// Pop a task from queues in priority order and create a lease for worker_id.
    /// Returns the task id if a task was leased.
    async fn dequeue_task(&self, worker_id: &str, worker_caps: &WorkerCapabilities)
    -> Option<Task>;

    /// Remove the lease for task id (worker finished or lost).
    async fn remove_lease(&self, task_id: &Uuid) -> Option<Lease>;

    /// Removes all the leases of a given worker.
    async fn remove_leases_of_worker(&self, worker_id: &str) -> anyhow::Result<Vec<Uuid>>;

    /// Remove the lease for task id (worker finished or lost).
    async fn get_all_leases(&self) -> HashMap<Uuid, Lease>;

    /// Store a task result (completed/failed).
    async fn store_result(&self, task_id: Uuid, result: TaskResult);

    /// Checks whether the storage contains a result for the given task id.
    async fn contains_result(&self, task_id: Uuid) -> bool;

    /// Get task result if present.
    async fn get_task_result(&self, task_id: Uuid) -> Option<TaskResult>;

    /// Gets all tasks result if present.
    async fn get_all_tasks(&self) -> Option<Vec<TaskResult>>;

    async fn mark_task_failed(
        &self,
        task_id: &Uuid,
        max_attempts: usize,
    ) -> anyhow::Result<(bool, String)>;

    async fn insert_worker_info_to_registry(&self, worker: WorkerInfo) -> anyhow::Result<()>;

    async fn exists_worker(&self, worker_id: &str) -> bool;

    async fn get_worker_from_registry(&self, worker_id: &str) -> Option<WorkerInfo>;

    async fn get_worker_registry(&self) -> HashMap<String, WorkerInfo>;

    async fn remove_worker_from_registry(&self, worker_id: &str) -> Option<WorkerInfo>;
}
