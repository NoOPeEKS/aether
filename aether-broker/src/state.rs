use std::collections::HashMap;

use aether_core::broker::storage::WorkerSession;
use aether_core::traits::Storage;
use tokio::sync::RwLock;

#[derive(Default, Debug)]
pub struct BrokerState<S>
where
    S: Storage,
{
    pub storage: S,
    pub worker_sessions: RwLock<HashMap<String, WorkerSession>>,
}

impl<S> BrokerState<S>
where
    S: Storage,
{
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            worker_sessions: RwLock::new(HashMap::new()),
        }
    }
}
