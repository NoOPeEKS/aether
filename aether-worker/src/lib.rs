mod loops;
mod state;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use aether_core::jrpc::{JsonRpcRequest, JsonRpcResponse, format_jrpc_message};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;

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
    pub tx: mpsc::Sender<String>,
    pub rx: mpsc::Receiver<String>,
    pub shutdown_token: CancellationToken,
}

impl Worker {
    pub fn new(id: &str, server_addr: &str, max_concurrent_tasks: usize) -> Self {
        // WE JUST DO STRINGS FOR NOW BC WE DON'T KNOW IF IT'S NOTIFICATION OR REQUEST SO WE JUST
        // SERIALIZE THEM INTO STRINGS.
        // TODO: Check if it would just be better to use an unbounded_channel.
        let (tx, rx) = mpsc::channel::<String>(999999999);
        Self {
            id: id.into(),
            max_concurrent_tasks,
            server_addr: server_addr.into(),
            state: Arc::new(WorkerState::new(id)),
            tx,
            rx,
            shutdown_token: CancellationToken::new(),
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let stream = TcpStream::connect(&self.server_addr).await?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        register_worker(&mut reader, &mut writer, &self.id).await?;

        // Heartbeat loop
        let heartbeat_tx = self.tx.clone();
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
            self.rx,
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
        let fetcher_tx = self.tx.clone();
        let fetcher_state = Arc::clone(&self.state);
        let fetcher_task = tokio::spawn(fetch_loop(
            fetcher_tx,
            fetcher_state,
            self.max_concurrent_tasks,
            self.shutdown_token.clone(),
        ));

        // Executor task
        let executor_tx = self.tx.clone();
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

        Ok(())
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
