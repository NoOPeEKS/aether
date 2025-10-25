use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

use aether_common::jrpc::{
    JsonRpcError, JsonRpcErrorCode, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
use aether_common::task::{Task, TaskResult};

use crate::state::{BrokerState, WorkerInfo};

const HEARTBEAT_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(10);
const CHECK_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(5);

pub async fn create_jrpc_server(state: BrokerState, port: usize) {
    let state = Arc::new(state);
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap_or_else(|_| panic!("Could not bind JRPC server to 0.0.0.0:{port}"));

    // Spawn a task that checks for heartbeats and updates worker states.
    let heartbeat_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CHECK_INTERVAL);
        loop {
            interval.tick().await;
            let now = tokio::time::Instant::now();
            let mut workers = heartbeat_state.worker_registry.write().await;
            for (_, winfo) in workers.iter_mut() {
                if now.duration_since(winfo.last_heartbeat) > HEARTBEAT_TIMEOUT {
                    winfo.active = false;
                }
            }
        }
    });

    loop {
        if let Ok((stream, addr)) = listener.accept().await {
            info!("[INFO] Accepted TCP connection from {}", addr);
            let state = Arc::clone(&state);
            tokio::spawn(handle_jrpc_connection(stream, state));
        } else {
            info!("[INFO] Could not accept an incoming connection");
        }
    }
}

async fn handle_jrpc_connection(stream: TcpStream, state: Arc<BrokerState>) {
    let (reader, mut writer) = TcpStream::into_split(stream);
    let mut reader = BufReader::new(reader);

    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Task to handle sending responses back to the client
    let _response_writer_task = tokio::spawn(async move {
        while let Some(response_msg) = response_rx.recv().await {
            if let Err(e) = writer.write_all(response_msg.as_bytes()).await {
                error!("[ERROR] Failed to write response to client connection: {e}");
                break;
            }
            if let Err(e) = writer.flush().await {
                error!("[ERROR] Failed to flush response to client connection: {e}");
                break;
            }
        }
        info!("[INFO] Response writer task for connection ending.");
    });

    loop {
        let mut line = String::new();

        let read = match reader.read_line(&mut line).await {
            // Should read "Content-Length: X\r\n"
            Ok(n) => n,
            Err(e) => {
                error!("[ERROR] Failed to read line from client: {e}");
                continue; // Try again if reading fails
            }
        };

        if read == 0 {
            info!("[INFO] Client closed connection (EOF)");
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
                Err(e) => {
                    error!("[ERROR] Invalid Content-Length: {e}");
                    // Invalid content length, just continue.
                    continue;
                }
            };

            let mut empty_line = String::new(); // Should read the following \r\n
            match reader.read_line(&mut empty_line).await {
                Ok(_) => {} // Read the empty line
                Err(e) => {
                    error!("[ERROR] Failed to read empty line: {e}");
                    continue;
                }
            }

            let mut message_body = vec![0; len];
            match reader.read_exact(&mut message_body).await {
                Ok(_) => {
                    let state_clone = Arc::clone(&state);
                    let response_tx_clone = response_tx.clone();


                    // Spawn a task to process the response in the background and let the thread
                    // continue iteration to keep reading messages!!!
                    tokio::spawn(async move {
                        match process_jsonrpc_message(&message_body, &state_clone).await {
                            Ok(Some(response)) => {
                                // Message was a request, need to send a response
                                match serde_json::to_string(&response) {
                                    Ok(res_str) => {
                                        let response_bytes = format!(
                                            "Content-Length: {}\r\n\r\n{}",
                                            res_str.len(),
                                            res_str
                                        );
                                        // Send the response string via the channel to the writer task
                                        if let Err(e) = response_tx_clone.send(response_bytes) {
                                            error!(
                                                "[ERROR] Failed to send response to writer task: {e}. Client connection likely closed."
                                            );
                                        }
                                    }
                                    Err(e) => error!("[ERROR] Failed to serialize response: {e}"),
                                }
                            }
                            Ok(None) => {
                                // Message was a notification, no response needed.
                            }
                            Err(e) => error!("[ERROR] Failed to process JSON-RPC message: {e}"),
                        }
                    });
                }
                Err(e) => {
                    error!("[ERROR] Failed to read full message body: {e}");
                    continue;
                }
            }
        } else {
            // Incorrect message, just continue
            error!("[ERROR] Received invalid header line: {}", line);
            continue;
        }
    }
    drop(response_tx);
}

async fn process_jsonrpc_message(
    message: &[u8],
    state: &BrokerState,
) -> anyhow::Result<Option<JsonRpcResponse>> {
    info!("[WARNING] Something hit the process_jsonrpc_message");
    let message = String::from_utf8_lossy(message);
    let message: serde_json::Value = serde_json::from_str(&message)?;
    if message.get("id").is_some() {
        let request: JsonRpcRequest = serde_json::from_value(message)?;
        info!("[INFO] Received a request of type {}", &request.method);

        if &request.method == "register_worker" {
            let register_req: RegisterWorkerRequestParams = serde_json::from_value(request.params)?;
            if !state
                .worker_registry
                .read()
                .await
                .contains_key(&register_req.worker_id)
            {
                info!(
                    "[INFO] Registered new worker with ID = {}",
                    &register_req.worker_id
                );
                state.worker_registry.write().await.insert(
                    register_req.worker_id.clone(),
                    WorkerInfo {
                        worker_id: register_req.worker_id,
                        last_heartbeat: tokio::time::Instant::now(),
                        active: true,
                    },
                );
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: Some(json!({"status": "registered"})),
                    error: None,
                }));
            } else {
                info!(
                    "[INFO] Could not register. A worker with ID = {} already exists.",
                    &register_req.worker_id
                );
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: JsonRpcErrorCode::InvalidParams,
                        message: "Worker ID has already been registered".into(),
                        data: None,
                    }),
                }));
            }
        } else if &request.method == "fetch_task" {
            let req_params: FetchTaskRequestParams = serde_json::from_value(request.params)?;
            info!("[INFO] Received a 'fetch_task' request");
            if !state
                .worker_registry
                .read()
                .await
                .contains_key(&req_params.worker_id)
            {
                info!("[INFO] Could not fetch task from a non-registered worker.");
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: JsonRpcErrorCode::InvalidRequest,
                        message: "Cannot fetch task from non-registered worker.".into(),
                        data: None,
                    }),
                }));
            }

            if let Some(winfo) = state
                .worker_registry
                .read()
                .await
                .get(&req_params.worker_id)
                && !winfo.active
            {
                info!("[INFO] Could not fetch task from an inactive worker.");
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: JsonRpcErrorCode::InvalidRequest,
                        message: "Cannot fetch task from an inactive worker".into(),
                        data: None,
                    }),
                }));
            }

            if let Some(task) = state.dequeue_task().await {
                info!("[INFO] Sending task to ID = {}", &req_params.worker_id);
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: Some(serde_json::to_value(FetchTaskResponseResult {
                        task: Some(task),
                    })?),
                    error: None,
                }));
            } else {
                info!("[INFO] Sending None task to ID = {}", &req_params.worker_id);
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: Some(serde_json::to_value(FetchTaskResponseResult {
                        task: None,
                    })?),
                    error: None,
                }));
            }
        }
    } else {
        // It's a notification
        let notification: JsonRpcNotification = serde_json::from_value(message)?;

        if &notification.method == "heartbeat" {
            if let Ok(heartbeat_params) =
                serde_json::from_value::<HeartbeatNotificationParams>(notification.params)
                && state
                    .worker_registry
                    .read()
                    .await
                    .contains_key(&heartbeat_params.worker_id)
                && let Some(worker_info) = state
                    .worker_registry
                    .write()
                    .await
                    .get_mut(&heartbeat_params.worker_id)
            {
                info!(
                    "[INFO] Heartbeat notification received from ID = {}",
                    &worker_info.worker_id
                );
                worker_info.last_heartbeat = tokio::time::Instant::now();
            }
        } else if &notification.method == "report_result"
            && let Ok(task_result) = serde_json::from_value::<TaskResult>(notification.params)
            && state.tasks.read().await.contains_key(&task_result.id)
        {
            info!("[INFO] Result from task ID = {} received.", task_result.id);
            state
                .tasks
                .write()
                .await
                .insert(task_result.id, task_result);
        }
    }
    Ok(None)
}

#[derive(Serialize, Deserialize, Debug)]
struct RegisterWorkerRequestParams {
    worker_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct FetchTaskRequestParams {
    worker_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct FetchTaskResponseResult {
    task: Option<Task>,
}

#[derive(Serialize, Deserialize, Debug)]
struct HeartbeatNotificationParams {
    worker_id: String,
}
