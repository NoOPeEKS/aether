use std::sync::Arc;

use aether_core::jrpc::{JsonRpcNotification, format_jrpc_message};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info, warn};

use crate::state::WorkerState;

pub async fn heartbeat_loop(heartbeat_tx: mpsc::Sender<String>, state: Arc<WorkerState>) {
    info!("[INFO] Starting heartbeat task");
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let heartbeat_notif = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: "heartbeat".into(),
            params: json!({
                "worker_id": state.id,
            }),
        };

        let msg = format_jrpc_message(heartbeat_notif).unwrap();

        match heartbeat_tx.try_send(msg) {
            Ok(()) => {
                info!("[INFO] Heartbeat sent.");
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Continue the loop potentially missing one heartbeat
                warn!("[WARNING] Heartbeat channel is full, skipping heartbeat.")
            }

            Err(mpsc::error::TrySendError::Closed(_)) => {
                // If writer has stopped just crash this task.
                error!("[ERROR] Heartbeat task: Writer channel closed.");
                break;
            }
        }
    }
}
