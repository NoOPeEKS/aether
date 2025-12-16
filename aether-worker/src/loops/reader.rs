use std::sync::Arc;

use aether_core::jrpc::{JsonRpcNotification, JsonRpcResponse};
use aether_core::task::Task;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::WorkerState;

pub async fn reader_loop(mut reader: BufReader<OwnedReadHalf>, state: Arc<WorkerState>) {
    info!("[INFO] Starting reader task");

    loop {
        let mut headers = String::new();
        let mut content_length: Option<usize> = None;

        // Read headers until empty line
        loop {
            headers.clear();
            let _n = match reader.read_line(&mut headers).await {
                Ok(0) => {
                    info!("[INFO] Server closed connection");
                    return;
                }
                Ok(n) => n,
                Err(e) => {
                    error!("[ERROR] Failed to read line from broker: {e}");
                    return;
                }
            };

            let trimmed = headers.trim();
            if trimmed.is_empty() {
                break;
            }

            if let Some(val) = trimmed.strip_prefix("Content-Length: ")
                && let Ok(len) = val.trim().parse::<usize>()
            {
                content_length = Some(len);
            }
        }

        let len = match content_length {
            Some(len) => len,
            None => {
                error!("[ERROR] No Content-Length header received");
                continue;
            }
        };

        let mut body = vec![0u8; len];
        if let Err(e) = reader.read_exact(&mut body).await {
            error!("[ERROR] Failed to read full response body: {e}");
            return;
        }

        let msg = String::from_utf8_lossy(&body);
        info!("[INFO] Received from broker: {}", msg);
        tokio::spawn(handle_server_message(msg.into(), Arc::clone(&state)));
    }
}

async fn handle_server_message(message: String, state: Arc<WorkerState>) {
    let message: serde_json::Value = serde_json::from_str(&message).unwrap();
    if message.get("id").is_some() {
        // It was a response.
        let response: JsonRpcResponse = serde_json::from_value(message).unwrap();
        if let Some(error) = response.error {
            // Something happened with the request and we got back an error. For now we just log it
            warn!(
                "[WARNING] Request with id {} got response with an error code: {}. Message: {}",
                response.id, error.code, error.message
            );
        } else {
            // Actual response
            if let Some(result) = response.result
                && let Some(task_val) = result.get("task")
                && let Ok(task) = serde_json::from_value::<Task>(task_val.clone())
            {
                // This was a response to a fetch task.
                let task_id = task.id;
                state.task_list.write().await.push_back(task);
                info!(
                    "[INFO] Got a 'fetch_task' response from server and queued task {} into worker queue",
                    task_id
                );
            }
        }
    } else {
        // It was a notification.
        // TODO: Check these unwraps.
        let notification: JsonRpcNotification = serde_json::from_value(message).unwrap();

        // TODO: Implement task cancellation flow with a channel.
        if &notification.method == "stop_execution" {
            let params: StopExecutionNotificationParams =
                serde_json::from_value(notification.params).unwrap();
            if let Some(running) = state.running_tasks.write().await.remove(&params.task_id) {
                _ = running.cancel_tx.send(());
                tokio::spawn(async move {
                    _ = running.handle.await;
                    warn!("[WARNING] Cancelled task {}", &params.task_id);
                });
            } else {
                warn!(
                    "[WARNING] stop_execution: task {} not found",
                    &params.task_id
                );
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct StopExecutionNotificationParams {
    task_id: Uuid,
}
