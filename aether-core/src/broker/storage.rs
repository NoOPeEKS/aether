use std::collections::{HashMap, VecDeque};

use redis::AsyncTypedCommands;
use redis::RedisError;
use tokio::sync::RwLock;
use tokio::time::Instant;
use uuid::Uuid;

use crate::capabilities::WorkerCapabilities;
use crate::task::{Lease, Task, TaskPriority, TaskResult, TaskStatus};
use crate::traits::Storage;

#[derive(Default)]
pub struct InMemoryStorage {
    pub high_prio: RwLock<VecDeque<Task>>,
    pub mid_prio: RwLock<VecDeque<Task>>,
    pub low_prio: RwLock<VecDeque<Task>>,
    pub results: RwLock<HashMap<Uuid, TaskResult>>,
    pub leases: RwLock<HashMap<Uuid, Lease>>,
}

async fn pop_compatible(
    queue: &tokio::sync::RwLock<VecDeque<Task>>,
    worker_caps: &WorkerCapabilities,
) -> Option<Task> {
    let mut queue = queue.write().await;

    if let Some((idx, _)) = queue
        .iter()
        .enumerate()
        .find(|(_, t)| worker_caps.supports(t.capabilities.clone()))
    {
        queue.remove(idx)
    } else {
        None
    }
}

#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    async fn enqueue_task(&self, task: Task) -> anyhow::Result<()> {
        match task.priority {
            TaskPriority::High => self.high_prio.write().await.push_back(task),
            TaskPriority::Medium => self.mid_prio.write().await.push_back(task),
            TaskPriority::Low => self.low_prio.write().await.push_back(task),
        }
        Ok(())
    }

    async fn dequeue_task(
        &self,
        worker_id: &str,
        worker_caps: &WorkerCapabilities,
    ) -> Option<Task> {
        let task = if let Some(t) = pop_compatible(&self.high_prio, worker_caps).await {
            Some(t)
        } else if let Some(t) = pop_compatible(&self.mid_prio, worker_caps).await {
            Some(t)
        } else {
            pop_compatible(&self.low_prio, worker_caps).await
        };

        match task {
            Some(t) => {
                let id = t.id;

                self.results.write().await.insert(
                    id,
                    TaskResult {
                        id,
                        name: t.name.clone(),
                        code_b64: t.code_b64.clone(),
                        result: None,
                        status: TaskStatus::Running,
                        capabilities: t.capabilities.clone(),
                    },
                );

                self.leases.write().await.insert(
                    id,
                    Lease {
                        worker_id: worker_id.to_owned(),
                        attempts: 0,
                        start_time: Instant::now(),
                    },
                );
                Some(t)
            }
            None => return None,
        }
    }

    async fn remove_lease(&self, task_id: &Uuid) -> Option<Lease> {
        self.leases.write().await.remove(task_id)
    }

    async fn remove_leases_of_worker(&self, worker_id: &str) -> anyhow::Result<Vec<Uuid>> {
        let mut leases = self.leases.write().await;

        let lease_ids: Vec<_> = leases
            .iter()
            .filter(|(_, lease)| lease.worker_id == worker_id)
            .map(|(l_id, _)| *l_id)
            .collect();

        _ = lease_ids.iter().filter_map(|id| leases.remove(id));
        drop(leases);

        Ok(lease_ids)
    }

    async fn store_result(&self, task_id: Uuid, result: TaskResult) {
        self.results.write().await.insert(task_id, result);
    }

    async fn contains_result(&self, task_id: Uuid) -> bool {
        self.results.read().await.contains_key(&task_id)
    }

    async fn get_task_result(&self, task_id: Uuid) -> Option<TaskResult> {
        self.results.read().await.get(&task_id).cloned()
    }

    async fn get_all_tasks(&self) -> Option<Vec<TaskResult>> {
        let tasks: Vec<TaskResult> = self.results.read().await.values().cloned().collect();
        if tasks.is_empty() { None } else { Some(tasks) }
    }

    async fn mark_task_failed(
        &self,
        task_id: &Uuid,
        max_attempts: usize,
    ) -> anyhow::Result<(bool, String)> {
        let mut leases = self.leases.write().await;
        let lease = leases
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Lease did not exist in storage"))?;
        lease.attempts += 1;
        let wid = lease.worker_id.clone();

        let mut results = self.results.write().await;
        let result = results
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Result did not exist in storage."))?;
        result.status = TaskStatus::Failed;
        drop(results);

        let too_many_attempts = lease.attempts >= max_attempts;
        Ok((too_many_attempts, wid))
    }

    async fn get_all_leases(&self) -> HashMap<Uuid, Lease> {
        self.leases.read().await.clone()
    }
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

pub struct RedisStorage {
    client: redis::Client,
    connection: redis::aio::MultiplexedConnection,
}

impl RedisStorage {
    pub async fn new(ip: &str, port: usize) -> Result<Self, RedisError> {
        let url = format!("redis://{ip}:{port}");
        let client = redis::Client::open(url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        Ok(Self { client, connection })
    }
}

#[async_trait::async_trait]
impl Storage for RedisStorage {
    async fn enqueue_task(&self, task: Task) -> anyhow::Result<()> {
        let mut conn = self.connection.clone();
        let json = serde_json::to_string(&task)?;
        let queue_key = match task.priority {
            TaskPriority::High => "task_queue:high",
            TaskPriority::Medium => "task_queue:medium",
            TaskPriority::Low => "task_queue:low",
        };
        conn.lpush(queue_key, json).await?;
        Ok(())
    }

    async fn dequeue_task(
        &self,
        worker_id: &str,
        worker_caps: &WorkerCapabilities,
    ) -> Option<Task> {
        let mut conn = self.connection.clone();
        let queues = ["task_queue:high", "task_queue:medium", "task_queue:low"];
        for queue_key in queues {
            let tasks = match conn.lrange(queue_key, 0, -1).await {
                Ok(t) => t,
                Err(_) => continue,
            };
            for task_json in tasks {
                let task: Task = match serde_json::from_str(&task_json) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if worker_caps.supports(task.capabilities.clone()) {
                    // Remove the task from the queue.
                    match conn.lrem(queue_key, 1, &task_json).await {
                        Ok(_) => {}
                        Err(_) => continue,
                    }
                    let result_key = format!("task_results:{}", task.id);
                    let result = TaskResult {
                        id: task.id,
                        name: task.name,
                        code_b64: task.code_b64,
                        result: None,
                        status: TaskStatus::Queued,
                        capabilities: task.capabilities,
                    };
                    // TODO: Check this unwrap.
                    let result_json = serde_json::to_string(&result).unwrap();
                    match conn.set(result_key, result_json).await {
                        Ok(_) => {}
                        Err(_) => continue,
                    }
                    let lease_key = format!("leases:{}", task.id);
                    let lease = Lease {
                        worker_id: worker_id.to_owned(),
                        attempts: 1,
                        start_time: Instant::now(),
                    };
                    // TODO: Check this unwrap.
                    let lease_json = serde_json::to_string(&lease).unwrap();
                    match conn.set(lease_key, lease_json).await {
                        Ok(_) => {}
                        Err(_) => continue,
                    }
                }
            }
        }
        None
    }

    async fn remove_lease(&self, task_id: &Uuid) -> Option<Lease> {
        let mut conn = self.connection.clone();
        let lease_key = format!("leases:{task_id}");
        if let Ok(Some(lease)) = conn.get(lease_key).await {
            if let Ok(lease) = serde_json::from_str::<Lease>(&lease) {
                return Some(lease);
            }
            return None;
        }
        None
    }

    async fn remove_leases_of_worker(&self, worker_id: &str) -> anyhow::Result<Vec<Uuid>> {
        todo!("Implement this function");
    }

    async fn get_all_leases(&self) -> HashMap<Uuid, Lease> {
        todo!("Implement this function");
    }

    async fn store_result(&self, task_id: Uuid, result: TaskResult) {
        let mut conn = self.connection.clone();
        let result_key = format!("task_results:{task_id}");
        if let Ok(result_json) = serde_json::to_string(&result) {
            conn.set(result_key, result_json).await.unwrap();
        }
    }

    async fn contains_result(&self, task_id: Uuid) -> bool {
        let mut conn = self.connection.clone();
        let result_key = format!("task_results:{task_id}");
        conn.exists(result_key).await.unwrap_or(false)
    }

    async fn get_task_result(&self, task_id: Uuid) -> Option<TaskResult> {
        let mut conn = self.connection.clone();
        let result_key = format!("task_results:{task_id}");
        let task_res = conn.get(result_key).await.unwrap_or(None);
        if let Some(tr) = task_res {
            // TODO: Check this unwrap.
            let res: TaskResult = serde_json::from_str(&tr).unwrap();
            return Some(res);
        }
        None
    }

    async fn get_all_tasks(&self) -> Option<Vec<TaskResult>> {
        todo!("Implement this function");
    }

    async fn mark_task_failed(
        &self,
        task_id: &Uuid,
        max_attempts: usize,
    ) -> anyhow::Result<(bool, String)> {
        todo!("Implement this function");
    }
}
