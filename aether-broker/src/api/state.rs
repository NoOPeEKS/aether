use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::api::types::{Task, TaskResult};

#[derive(Clone)]
pub struct AppState {
    pub queue: Arc<RwLock<HashMap<Uuid, Task>>>,
    pub results: Arc<RwLock<HashMap<Uuid, TaskResult>>>,
}
