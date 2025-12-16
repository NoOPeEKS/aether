use std::sync::Arc;

use aether_core::jrpc::{JsonRpcNotification, format_jrpc_message};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::jrpc::message::process_jsonrpc_message;
use crate::jrpc::params::*;
use crate::state::BrokerState;

const HEARTBEAT_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(10);
const CHECK_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(5);
const MAX_EXECUTION_TIME: tokio::time::Duration = tokio::time::Duration::from_secs(30);

pub async fn create_jrpc_server(state: Arc<BrokerState>, port: usize) {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap_or_else(|_| panic!("Could not bind JRPC server to 0.0.0.0:{port}"));

    // Spawn a task that checks for heartbeats and updates worker states.
    let heartbeat_state = Arc::clone(&state);
    tokio::spawn(handle_heartbeats(heartbeat_state));

    let timeouts_state = Arc::clone(&state);
    tokio::spawn(handle_timeouts(timeouts_state));

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

async fn handle_heartbeats(state: Arc<BrokerState>) {
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    loop {
        interval.tick().await;
        let now = tokio::time::Instant::now();
        let mut workers = state.worker_registry.write().await;
        for (_, winfo) in workers.iter_mut() {
            if now.duration_since(winfo.last_heartbeat) > HEARTBEAT_TIMEOUT {
                winfo.active = false;
            }
        }
    }
}

async fn handle_timeouts(state: Arc<BrokerState>) {
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    loop {
        interval.tick().await;
        let now = tokio::time::Instant::now();
        let mut leases = state.leases.write().await;
        for (task_id, lease) in leases.iter_mut() {
            if now.duration_since(lease.start_time) > MAX_EXECUTION_TIME {
                warn!(
                    "[WARNING] Task {} exceeded maximum execution time. Cancelling...",
                    task_id
                );
                // TODO: Check this unwrap, though it should never fail.
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0".into(),
                    method: "stop_execution".into(),
                    params: serde_json::to_value(StopExecutionNotificationParams {
                        task_id: *task_id,
                    })
                    .unwrap(),
                };
                let worker_id = &lease.worker_id;
                if let Some(wsession) = state.worker_sessions.read().await.get(worker_id) {
                    // TODO: Check this unwraps.
                    wsession
                        .sender
                        .send(format_jrpc_message(notif).unwrap())
                        .unwrap();
                }
            }
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
                    let response_tx_clone1 = response_tx.clone();
                    let response_tx_clone2 = response_tx.clone();

                    // Spawn a task to process the response in the background and let the thread
                    // continue iteration to keep reading messages!!!
                    tokio::spawn(async move {
                        match process_jsonrpc_message(
                            &message_body,
                            &state_clone,
                            response_tx_clone1,
                        )
                        .await
                        {
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
                                        if let Err(e) = response_tx_clone2.send(response_bytes) {
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
