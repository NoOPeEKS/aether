use crate::jrpc::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::state::BrokerState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::info;

pub async fn create_jrpc_server(state: BrokerState, port: usize) {
    let state = Arc::new(state);
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap_or_else(|_| panic!("Could not bind JRPC server to 0.0.0.0:{port}"));

    let workers: HashSet<String> = HashSet::new();
    let worker_list = Arc::new(RwLock::new(workers));

    loop {
        if let Ok((stream, addr)) = listener.accept().await {
            info!("[INFO] Accepted TCP connection from {}", addr);
            let state = Arc::clone(&state);
            let workers = Arc::clone(&worker_list);
            tokio::spawn(handle_jrpc_connection(stream, state, workers));
        } else {
            info!("[INFO] Could not accept an incoming connection");
        }
    }
}

async fn handle_jrpc_connection(
    stream: TcpStream,
    state: Arc<BrokerState>,
    workers: Arc<RwLock<HashSet<String>>>,
) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();

        let read = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(_) => continue,
        };

        if read == 0 {
            break;
        }

        if line.starts_with("Content-Length: ") {
            // Correct message
            let len = match line
                .trim_start_matches("Content-Length: ")
                .trim()
                .parse::<usize>()
            {
                Ok(len) => len,
                Err(_) => {
                    // Invalid content length, just continue.
                    continue;
                }
            };

            let mut empty_line = String::new();
            if reader.read_line(&mut empty_line).await.is_err() {
                continue;
            }

            let mut message_body = vec![0; len];
            if reader.read_exact(&mut message_body).await.is_err() {
                continue;
            }

            let workers = Arc::clone(&workers);
            match process_jsonrpc_message(&message_body, &state, workers).await {
                Ok(_) => {}
                Err(_) => continue,
            }
        } else {
            // Incorrect message, just continue
            continue;
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct RegisterWorkerRequest {
    worker_id: String,
}

async fn process_jsonrpc_message(
    message: &[u8],
    state: &BrokerState,
    workers: Arc<RwLock<HashSet<String>>>,
) -> anyhow::Result<()> {
    let message = String::from_utf8_lossy(message);
    let message: serde_json::Value = serde_json::from_str(&message)?;
    if message.get("id").is_some() {
        // It's a request
        let request: JsonRpcRequest = serde_json::from_value(message)?;
        info!("[INFO] Received a request of type {}", &request.method);
        if &request.method == "register_worker" {
            let register_req: RegisterWorkerRequest = serde_json::from_value(request.params)?;
            if !workers.read().await.contains(&register_req.worker_id) {
                workers.write().await.insert(register_req.worker_id);
            } else {
                // TODO: Build custom Error types to know why the processing fails.
                anyhow::bail!("This worker ID is already registered!");
            }
        }
    } else {
        // It's a notification
        let notification: JsonRpcNotification = serde_json::from_value(message)?;
        if &notification.method == "heartbeat" {} // Implement heartbeat
    }
    Ok(())
}
