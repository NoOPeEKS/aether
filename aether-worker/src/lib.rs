mod loops;
mod state;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use aether_core::capabilities::WorkerCapabilities;
use aether_core::jrpc::{JsonRpcRequest, JsonRpcResponse, format_jrpc_message};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::loops::executor::executor_loop;
use crate::loops::fetch::fetch_loop;
use crate::loops::heartbeat::heartbeat_loop;
use crate::loops::reader::reader_loop;
use crate::loops::writer::writer_loop;
use crate::state::WorkerState;

static ID: AtomicUsize = AtomicUsize::new(1);

fn next_id() -> usize {
    ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

pub struct Worker {
    pub id: String,
    pub max_concurrent_tasks: usize,
    pub server_addr: String,
    pub state: Arc<WorkerState>,
    pub shutdown_token: CancellationToken,
    pub capabilities: WorkerCapabilities,
}

impl Worker {
    pub fn new(
        id: &str,
        server_addr: &str,
        max_concurrent_tasks: usize,
        capabilities: WorkerCapabilities,
    ) -> Self {
        Self {
            id: id.into(),
            max_concurrent_tasks,
            server_addr: server_addr.into(),
            state: Arc::new(WorkerState::new(id)),
            shutdown_token: CancellationToken::new(),
            capabilities,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(60);
        loop {
            if self.shutdown_token.is_cancelled() {
                break;
            }
            // TODO: Check if it would just be better to use an unbounded_channel.
            let (tx, rx) = mpsc::channel::<String>(999999999);
            match TcpStream::connect(&self.server_addr).await {
                Ok(stream) => {
                    backoff = Duration::from_secs(1);
                    let (reader, mut writer) = stream.into_split();
                    let mut reader = BufReader::new(reader);

                    match register_worker(
                        &mut reader,
                        &mut writer,
                        &self.id,
                        self.capabilities.clone(),
                    )
                    .await
                    {
                        Ok(_) => {
                            // We clear them just in case because reconnections can cause issues.
                            // In the future might wanna change this behavior?
                            self.state.task_list.write().await.clear();
                            self.state.running_tasks.write().await.clear();

                            // Heartbeat loop
                            let heartbeat_tx = tx.clone();
                            let heartbeat_state = Arc::clone(&self.state);
                            let heartbeat_task = tokio::spawn(heartbeat_loop(
                                heartbeat_tx,
                                heartbeat_state,
                                self.shutdown_token.clone(),
                            ));

                            // Writer loop
                            let worker_id = self.id.clone();
                            let writer_task = tokio::spawn(writer_loop(
                                worker_id,
                                rx,
                                writer,
                                self.shutdown_token.clone(),
                            ));

                            // Reader task
                            let _reader_state = Arc::clone(&self.state);
                            let reader_task = tokio::spawn(reader_loop(
                                reader,
                                _reader_state,
                                self.shutdown_token.clone(),
                            ));

                            // Fetch task
                            let fetcher_tx = tx.clone();
                            let fetcher_state = Arc::clone(&self.state);
                            let fetcher_task = tokio::spawn(fetch_loop(
                                fetcher_tx,
                                fetcher_state,
                                self.max_concurrent_tasks,
                                self.shutdown_token.clone(),
                            ));

                            // Executor task
                            let executor_tx = tx.clone();
                            let executor_state = Arc::clone(&self.state);
                            let executor_task = tokio::spawn(executor_loop(
                                executor_tx,
                                executor_state,
                                self.shutdown_token.clone(),
                            ));

                            tokio::select! {
                                _ = writer_task => error!("[ERROR] Writer task crashed."),
                                _ = reader_task => error!("[ERROR] Reader task crashed."),
                                _ = fetcher_task => error!("[ERROR] Fetcher task crashed."),
                                _ = heartbeat_task => error!("[ERROR] Heartbeat task crashed."),
                                _ = executor_task => error!("[ERROR] Executor task crashed."),
                            };
                        }
                        Err(_) => {
                            error!("[ERROR] Registration failed. Reconnecting...");
                        }
                    }
                }
                Err(_) => {
                    error!("[ERROR] Connection failed. Retrying...");
                }
            }
            info!(
                "[INFO] Waiting for {:?} seconds before reconnecting...",
                backoff
            );
            tokio::select! {
                _ = tokio::time::sleep(backoff) => {}
                _ = self.shutdown_token.cancelled() => {
                    return Ok(());
                }
            }
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
        Ok(())
    }
}

async fn register_worker(
    reader: &mut BufReader<OwnedReadHalf>,
    writer: &mut OwnedWriteHalf,
    worker_id: &str,
    capabilities: WorkerCapabilities,
) -> anyhow::Result<()> {
    let capabilities = match serde_json::to_value(capabilities) {
        Ok(caps) => caps,
        Err(_) => json!({"gpu": false, "arch": "x86_64"}),
    };
    let register_worker_body = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: format!("{}", next_id()),
        method: "register_worker".into(),
        params: json!({
            "worker_id": worker_id.to_string(),
            "capabilities": capabilities,
        }),
    };
    let message = format_jrpc_message(register_worker_body)?;
    writer.write_all(message.as_bytes()).await?;

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
