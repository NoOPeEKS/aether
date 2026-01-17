use std::collections::{HashMap, HashSet, VecDeque};
use std::str::FromStr;
use std::time::SystemTime;

use redis::{AsyncTypedCommands, RedisError};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::auth::User;
use crate::capabilities::WorkerCapabilities;
use crate::task::{Lease, Task, TaskPriority, TaskResult, TaskStatus};
use crate::traits::Storage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub last_heartbeat: SystemTime,
    pub active: bool,
    pub capabilities: WorkerCapabilities,
}

#[derive(Clone, Debug)]
pub struct WorkerSession {
    pub sender: tokio::sync::mpsc::UnboundedSender<String>,
    pub connected_at: tokio::time::Instant,
}

#[derive(Default)]
pub struct InMemoryStorage {
    pub high_prio: RwLock<VecDeque<Task>>,
    pub mid_prio: RwLock<VecDeque<Task>>,
    pub low_prio: RwLock<VecDeque<Task>>,
    pub results: RwLock<HashMap<Uuid, TaskResult>>,
    pub leases: RwLock<HashMap<Uuid, Lease>>,
    pub worker_registry: RwLock<HashMap<String, WorkerInfo>>,
    pub worker_sessions: RwLock<HashMap<String, WorkerSession>>,
    pub users: RwLock<HashSet<User>>,
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
                        start_time: SystemTime::now(),
                    },
                );
                Some(t)
            }
            None => return None,
        }
    }

    async fn get_lease(&self, task_id: &Uuid) -> Option<Lease> {
        self.leases.read().await.get(task_id).cloned()
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

    async fn insert_worker_info_to_registry(&self, worker: WorkerInfo) -> anyhow::Result<()> {
        self.worker_registry
            .write()
            .await
            .insert(worker.worker_id.clone(), worker)
            .ok_or(anyhow::anyhow!(
                "Error occured during insertion of worker info to registry"
            ))?;
        Ok(())
    }

    async fn get_worker_from_registry(&self, worker_id: &str) -> Option<WorkerInfo> {
        self.worker_registry.read().await.get(worker_id).cloned()
    }

    async fn get_worker_registry(&self) -> HashMap<String, WorkerInfo> {
        self.worker_registry.read().await.clone()
    }

    async fn exists_worker(&self, worker_id: &str) -> bool {
        self.worker_registry
            .read()
            .await
            .iter()
            .any(|(wid, _)| wid == worker_id)
    }

    async fn remove_worker_from_registry(&self, worker_id: &str) -> Option<WorkerInfo> {
        self.worker_registry.write().await.remove(worker_id)
    }

    async fn create_user(&self, user: User) -> anyhow::Result<()> {
        self.users.write().await.insert(user);
        Ok(())
    }

    async fn get_user(&self, username: &str) -> anyhow::Result<Option<User>> {
        let user = self
            .users
            .read()
            .await
            .iter()
            .find(|user| user.name == username)
            .cloned();
        Ok(user)
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
    _client: redis::Client,
    connection: redis::aio::MultiplexedConnection,
}

impl RedisStorage {
    pub async fn new(ip: &str, port: usize) -> Result<Self, RedisError> {
        let url = format!("redis://{ip}:{port}");
        let client = redis::Client::open(url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        Ok(Self {
            _client: client,
            connection,
        })
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
                        name: task.name.clone(),
                        code_b64: task.code_b64.clone(),
                        result: None,
                        status: TaskStatus::Queued,
                        capabilities: task.capabilities.clone(),
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
                        start_time: SystemTime::now(),
                    };
                    // TODO: Check this unwrap.
                    let lease_json = serde_json::to_string(&lease).unwrap();
                    match conn.set(lease_key, lease_json).await {
                        Ok(_) => {}
                        Err(_) => continue,
                    }
                    return Some(task);
                }
            }
        }
        None
    }

    async fn get_lease(&self, task_id: &Uuid) -> Option<Lease> {
        let mut conn = self.connection.clone();
        let lease_key = format!("leases:{task_id}");
        if let Ok(Some(lease)) = conn.get(lease_key).await
            && let Ok(lease) = serde_json::from_str::<Lease>(&lease)
        {
            return Some(lease);
        }
        None
    }

    async fn remove_lease(&self, task_id: &Uuid) -> Option<Lease> {
        let mut conn = self.connection.clone();
        let lease_key = format!("leases:{task_id}");
        if let Ok(Some(lease)) = conn.get(&lease_key).await {
            if let Ok(lease) = serde_json::from_str::<Lease>(&lease)
                && let Ok(_) = conn.del(&lease_key).await
            {
                return Some(lease);
            }
            return None;
        }
        None
    }

    async fn remove_leases_of_worker(&self, worker_id: &str) -> anyhow::Result<Vec<Uuid>> {
        let mut conn = self.connection.clone();
        let lease_keys = conn.keys("leases:*").await?;
        let mut removed_ids = Vec::new();
        for key in &lease_keys {
            if let Ok(Some(lease_json)) = conn.get(key).await
                && let Ok(lease_data) = serde_json::from_str::<Lease>(&lease_json)
                && lease_data.worker_id == *worker_id
                && let Some(task_id_str) = key.strip_prefix("leases:")
                && let Ok(task_id) = Uuid::parse_str(task_id_str)
            {
                conn.del(key).await?;
                removed_ids.push(task_id);
            }
        }
        Ok(removed_ids)
    }

    async fn get_all_leases(&self) -> HashMap<Uuid, Lease> {
        let mut conn = self.connection.clone();
        let lease_keys = match conn.keys("leases:*").await {
            Ok(keys) => keys,
            Err(_) => return HashMap::new(),
        };
        let mut leases = HashMap::new();
        for key in lease_keys {
            if let Some(task_id) = key.clone().strip_prefix("leases:")
                && let Ok(Some(lease_json)) = conn.get(key).await
                && let Ok(lease) = serde_json::from_str::<Lease>(&lease_json)
                && let Ok(task_uid) = Uuid::from_str(task_id)
            {
                leases.insert(task_uid, lease);
            }
        }

        leases
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
        let mut conn = self.connection.clone();
        let mut tasks = Vec::new();

        if let Ok(results_keys) = conn.keys("task_results:*").await {
            for key in results_keys {
                if let Ok(Some(task_result)) = conn.get(key).await
                    && let Ok(result) = serde_json::from_str::<TaskResult>(&task_result)
                {
                    tasks.push(result);
                }
            }
        }

        if tasks.is_empty() { None } else { Some(tasks) }
    }

    async fn mark_task_failed(
        &self,
        task_id: &Uuid,
        max_attempts: usize,
    ) -> anyhow::Result<(bool, String)> {
        let mut conn = self.connection.clone();
        let lease_key = format!("leases:{task_id}");
        let result_key = format!("task_results:{task_id}");

        if let Ok(Some(lease_json)) = conn.get(&lease_key).await {
            let mut lease: Lease = serde_json::from_str(&lease_json)?;
            lease.attempts += 1;
            conn.set(&lease_key, serde_json::to_string(&lease)?).await?;

            if let Ok(Some(result_json)) = conn.get(&result_key).await {
                let mut result: TaskResult = serde_json::from_str(&result_json)?;
                result.status = TaskStatus::Failed;
                conn.set(&result_key, serde_json::to_string(&result)?)
                    .await?;
                let too_many_attempts = lease.attempts >= max_attempts;
                return Ok((too_many_attempts, lease.worker_id));
            } else {
                anyhow::bail!("Result did not exist in storage");
            }
        } else {
            anyhow::bail!("lease did not exist in storage");
        }
    }

    async fn insert_worker_info_to_registry(&self, worker: WorkerInfo) -> anyhow::Result<()> {
        let mut conn = self.connection.clone();
        let wid = worker.worker_id.clone();
        let registry_key = format!("worker_registry:{wid}");
        conn.set(registry_key, serde_json::to_string(&worker)?)
            .await?;
        Ok(())
    }

    async fn get_worker_from_registry(&self, worker_id: &str) -> Option<WorkerInfo> {
        let mut conn = self.connection.clone();
        let registry_key = format!("worker_registry:{worker_id}");
        if let Ok(Some(winfo_json)) = conn.get(&registry_key).await {
            if let Ok(winfo) = serde_json::from_str::<WorkerInfo>(&winfo_json) {
                return Some(winfo);
            } else {
                return None;
            }
        }
        None
    }

    async fn get_worker_registry(&self) -> HashMap<String, WorkerInfo> {
        let mut conn = self.connection.clone();
        let mut worker_registry: HashMap<String, WorkerInfo> = HashMap::new();
        if let Ok(keys) = conn.keys("worker_registry:*").await {
            for key in keys {
                if let Some(wid) = key.strip_prefix("worker_registry:")
                    && let Ok(Some(winfo_json)) = conn.get(wid).await
                    && let Ok(winfo) = serde_json::from_str::<WorkerInfo>(&winfo_json)
                {
                    worker_registry.insert(wid.to_string(), winfo);
                }
            }
        }
        worker_registry
    }

    async fn exists_worker(&self, worker_id: &str) -> bool {
        let mut conn = self.connection.clone();
        let registry_key = format!("worker_registry:{worker_id}");
        conn.exists(registry_key).await.unwrap_or(false)
    }

    async fn remove_worker_from_registry(&self, worker_id: &str) -> Option<WorkerInfo> {
        let mut conn = self.connection.clone();
        let registry_key = format!("worker_registry:{worker_id}");
        if let Some(winfo) = self.get_worker_from_registry(worker_id).await {
            let del_res = conn.del(registry_key).await;
            if del_res.is_err() {
                return None;
            }
            return Some(winfo);
        }
        None
    }

    async fn create_user(&self, user: User) -> anyhow::Result<()> {
        let mut conn = self.connection.clone();
        let users_key = format!("users:{}", user.name);
        if let Ok(true) = conn.exists(&users_key).await {
            anyhow::bail!("User already exists!");
        }
        conn.set(users_key, serde_json::to_string(&user)?).await?;
        Ok(())
    }

    async fn get_user(&self, username: &str) -> anyhow::Result<Option<User>> {
        let mut conn = self.connection.clone();
        let user_key = format!("users:{username}");
        if let Ok(Some(user_str)) = conn.get(user_key).await {
            let user: User = serde_json::from_str(&user_str)?;
            return Ok(Some(user));
        }
        Ok(None)
    }
}
