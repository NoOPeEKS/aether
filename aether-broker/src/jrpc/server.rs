use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

use crate::jrpc::protocol::{
    JsonRpcError, JsonRpcErrorCode, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
use crate::state::{BrokerState, Task, TaskResult, WorkerInfo};

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

            match process_jsonrpc_message(&message_body, &state).await {
                Ok(Some(response)) => {
                    // Message was a request.
                    let res = serde_json::to_string(&response);
                    match res {
                        Ok(res_str) => {
                            let response_bytes =
                                format!("Content-Length: {}\r\n\r\n{}", res_str.len(), res_str);
                            writer.write_all(response_bytes.as_bytes()).await.unwrap();
                        }
                        Err(_) => continue,
                    }
                }
                Ok(None) => continue, // Was just a notification.
                Err(_) => continue,
            }
        } else {
            // Incorrect message, just continue
            continue;
        }
    }
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

async fn process_jsonrpc_message(
    message: &[u8],
    state: &BrokerState,
) -> anyhow::Result<Option<JsonRpcResponse>> {
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
            if !state
                .worker_registry
                .read()
                .await
                .contains_key(&req_params.worker_id)
            {
                anyhow::bail!("Cannot fetch task from an unregistered worker.");
            }

            if let Some(winfo) = state
                .worker_registry
                .read()
                .await
                .get(&req_params.worker_id)
                && !winfo.active
            {
                anyhow::bail!("Cannot fetch task from an inactive worker.");
            }

            if let Some(task) = state.dequeue_task().await {
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: Some(serde_json::to_value(FetchTaskResponseResult {
                        task: Some(task),
                    })?),
                    error: None,
                }));
            } else {
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
                worker_info.last_heartbeat = tokio::time::Instant::now();
            }
        } else if &notification.method == "report_result"
            && let Ok(task_result) = serde_json::from_value::<TaskResult>(notification.params)
            && state.tasks.read().await.contains_key(&task_result.id)
        {
            state
                .tasks
                .write()
                .await
                .insert(task_result.id, task_result);
        }
    }
    Ok(None)
}
