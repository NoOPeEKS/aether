use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicUsize},
};

use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::{RwLock, mpsc},
    time::Duration,
};
use tracing::{error, info, warn};
use uuid::Uuid;

use aether_common::jrpc::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use aether_common::task::{Task, TaskResult, TaskStatus};

static ID: AtomicUsize = AtomicUsize::new(1);

fn next_id() -> usize {
    ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

struct WorkerState {
    id: String,
    task_list: RwLock<HashMap<Uuid, Task>>,
}

impl WorkerState {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            task_list: RwLock::new(HashMap::new()),
        }
    }
}

async fn register_worker(
    reader: &mut BufReader<OwnedReadHalf>,
    writer: &mut OwnedWriteHalf,
    worker_id: &str,
) -> anyhow::Result<()> {
    let register_worker_body = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: format!("{}", next_id()),
        method: "register_worker".into(),
        params: json!({
            "worker_id": worker_id.to_string(),
        }),
    };
    let body = serde_json::to_string(&register_worker_body)?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
        .await?;

    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.starts_with("Content-Length: ") {
        let len = line
            .trim_start_matches("Content-Length: ")
            .trim()
            .parse::<usize>()?;

        reader.read_line(&mut line).await?; // Read empty line.
        let mut body = vec![0; len];
        reader.read_exact(&mut body).await?;

        let response: JsonRpcResponse = serde_json::from_slice(&body)?;
        if let Some(res) = response.result
            && res == json!({"status": "registered"})
        {
            Ok(())
        } else {
            anyhow::bail!("Register_worker response was not correct.");
        }
    } else {
        anyhow::bail!("Could not read bytes of register_worker response");
    }
}

pub async fn run_app(
    remote_rpc_server_ip: &str,
    worker_id: &str,
    max_concurrent_tasks: usize,
) -> anyhow::Result<()> {
    let worker_state = Arc::new(WorkerState::new(worker_id));
    let stream = TcpStream::connect(remote_rpc_server_ip).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let (tx, mut rx) = mpsc::channel::<String>(9999999999); // WE DO STRINGS FOR NOW BECAUSE WE DON'T KNOW
    // IF NOTIFICATION OR REQUEST. JUST SERIALIZE THEM.

    register_worker(&mut reader, &mut writer, worker_id).await?;

    // Heartbeat task
    let heartbeat_tx = tx.clone();
    let heartbeat_state = Arc::clone(&worker_state);
    let heartbeat_task = tokio::spawn(async move {
        info!("[INFO] Starting heartbeat task");
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let heartbeat_notif = JsonRpcNotification {
                jsonrpc: "2.0".into(),
                method: "heartbeat".into(),
                params: json!({
                    "worker_id": heartbeat_state.id,
                }),
            };
            let body = serde_json::to_string(&heartbeat_notif).unwrap();
            let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

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
    });

    // Writer task
    let writer_task = tokio::spawn(async move {
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
                    error!(
                        "[ERROR] Timed out while trying to write/flush message to socket."
                    );
                    break;
                }
            }
        }
        info!("[INFO] Writer task ending");
    });

    // Reader task
    let _reader_state = Arc::clone(&worker_state);
    let reader_task = tokio::spawn(async move {
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
        }
    });

    let fetcher_tx = tx.clone();
    let fetcher_state = Arc::clone(&worker_state);
    let fetcher_task = tokio::spawn(async move {
        info!("[INFO] Starting fetching task");
        let mut interval = tokio::time::interval(Duration::from_secs(7));
        interval.tick().await;
        loop {
            interval.tick().await;
            if fetcher_state.task_list.read().await.len() < max_concurrent_tasks {
                let fetch_task_msg = JsonRpcRequest {
                    jsonrpc: "2.0".into(),
                    id: next_id().to_string(),
                    method: "fetch_task".into(),
                    params: json!({
                        "worker_id": fetcher_state.id,
                    }),
                };
                let fetch_task_body = serde_json::to_string(&fetch_task_msg).unwrap();
                let msg = format!(
                    "Content-Length: {}\r\n\r\n{}",
                    fetch_task_body.len(),
                    fetch_task_body
                );

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
    });

    tokio::select! {
        _ = writer_task => error!("[ERROR] Writer task crashed."),
        _ = reader_task => error!("[ERROR] Reader task crashed."),
        _ = fetcher_task => error!("[ERROR] Fetcher task crashed."),
        _ = heartbeat_task => error!("[ERROR] Heartbeat task crashed."),
    };
    Ok(())
}
