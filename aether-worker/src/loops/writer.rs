use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info};

pub async fn writer_loop(mut rx: mpsc::Receiver<String>, mut writer: OwnedWriteHalf) {
    info!("[INFO] Starting writer task");
    loop {
        let msg = match rx.recv().await {
            Some(m) => m,
            None => {
                info!("[INFO] Stopping writer task due to channel closed.");
                break;
            }
        };

        let write_result = tokio::time::timeout(Duration::from_secs(10), async {
            writer.write_all(msg.as_bytes()).await?;
            writer.flush().await?;
            Ok::<(), std::io::Error>(())
        })
        .await;

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
