use aether_core::jrpc::{JsonRpcNotification, format_jrpc_message};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub async fn writer_loop(
    worker_id: String,
    mut rx: mpsc::Receiver<String>,
    mut writer: OwnedWriteHalf,
    shutdown_token: CancellationToken,
) {
    info!("[INFO] Starting writer task");
    loop {
        let thing = tokio::select! {
            _ = shutdown_token.cancelled() => {
                info!("[INFO] Writer task received cancellation signal. (During msg recv)");
                let cancel_notif = JsonRpcNotification {
                    jsonrpc: "2.0".into(),
                    method: "worker_shutdown".into(),
                    params: json!({"worker_id": worker_id}),
                };
                // SAFETY: This should never fail because JsonRpcNotification is Serializable.
                let shutdown_message = format_jrpc_message(cancel_notif).unwrap();
                if writer.write_all(shutdown_message.as_bytes()).await.is_err() {
                    warn!("[WARNING] Failed to write all the shutdown notification to broker.");
                }
                if writer.flush().await.is_err() {
                    warn!("[WARNING] Failed to flush the shutdown notification to broker.");
                }
                return;
            }
            msg = rx.recv() => {
                match msg {
                    Some(m) => m,
                    None => {
                        info!("[INFO] Stopping writer task due to channel closed.");
                        break;
                    }
                }
            }
        };

        let write_result = tokio::select! {
            _ = shutdown_token.cancelled() => {
                // TODO: Send shutdown message to broker before dying.
                info!("[INFO] Writer task received cancellation signal. (During msg recv)");
                return;
            }
            res = tokio::time::timeout(Duration::from_secs(10), async {
                    writer.write_all(thing.as_bytes()).await?;
                    writer.flush().await?;
                    Ok::<(), std::io::Error>(())
            }) => {
                res
            }
        };

        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                error!("[ERROR] Failed to write/flush message to socket within timeout: {e}");
                break;
            }
            Err(_) => {
                // Timeout elapsed
                error!("[ERROR] Timed out while trying to write/flush message to socket.");
                break;
            }
        }
    }
    info!("[INFO] Writer task ending");
}
