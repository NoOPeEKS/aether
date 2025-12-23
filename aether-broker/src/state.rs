use std::collections::HashMap;

use aether_core::{capabilities::WorkerCapabilities, traits::Storage};
use tokio::sync::RwLock;
use tokio::time::Instant;

#[derive(Clone, Debug)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub last_heartbeat: Instant,
    pub active: bool,
    pub capabilities: WorkerCapabilities,
}

#[derive(Clone, Debug)]
pub struct WorkerSession {
    pub sender: tokio::sync::mpsc::UnboundedSender<String>,
    pub connected_at: tokio::time::Instant,
}

#[derive(Default, Debug)]
pub struct BrokerState<S>
where
    S: Storage,
{
    pub storage: S,
    pub worker_registry: RwLock<HashMap<String, WorkerInfo>>,
    pub worker_sessions: RwLock<HashMap<String, WorkerSession>>,
}

impl<S> BrokerState<S>
where
    S: Storage,
{
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            worker_registry: RwLock::new(HashMap::new()),
            worker_sessions: RwLock::new(HashMap::new()),
        }
    }
}
