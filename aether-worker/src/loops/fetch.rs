use std::sync::Arc;

use aether_common::jrpc::{JsonRpcRequest, format_jrpc_message};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info, warn};

use crate::next_id;
use crate::state::WorkerState;

pub async fn fetch_loop(
    fetcher_tx: mpsc::Sender<String>,
    state: Arc<WorkerState>,
    max_concurrent_tasks: usize,
) {
    info!("[INFO] Starting fetching task");
    let mut interval = tokio::time::interval(Duration::from_secs(7));
    interval.tick().await;
    loop {
        interval.tick().await;
        if state.task_list.read().await.len() < max_concurrent_tasks {
            let fetch_task_msg = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: next_id().to_string(),
                method: "fetch_task".into(),
                params: json!({
                    "worker_id": state.id,
                }),
            };

            let msg = format_jrpc_message(fetch_task_msg).unwrap();

            match fetcher_tx.try_send(msg) {
                Ok(()) => {
                    info!("[INFO] Fetch_task request sent.");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!("[WARNING] Fetch task channel is full, skipping fetch_task attempt.");
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    error!("[ERROR] Fetcher task: Writer channel closed.");
                    break;
                }
            }
        }
    }
}
