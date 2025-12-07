use std::{
    collections::{HashMap, VecDeque},
    process::{ExitStatus, Stdio},
    sync::{Arc, atomic::AtomicUsize},
};

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    process::Command,
    sync::{RwLock, mpsc, oneshot},
    task::JoinHandle,
    time::Duration,
};
use tracing::{error, info, warn};
use uuid::Uuid;

use aether_common::jrpc::{
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, format_jrpc_message,
};
use aether_common::task::{Task, TaskResult, TaskStatus};

static ID: AtomicUsize = AtomicUsize::new(1);

fn next_id() -> usize {
    ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

struct WorkerState {
    id: String,
    task_list: RwLock<VecDeque<Task>>,
    running_tasks: RwLock<HashMap<Uuid, RunningTask>>,
}

impl WorkerState {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            task_list: RwLock::new(VecDeque::new()),
            running_tasks: RwLock::new(HashMap::new()),
        }
    }
}

struct RunningTask {
    handle: JoinHandle<()>,
    cancel_tx: oneshot::Sender<()>,
}

#[derive(Deserialize, Serialize)]
struct PythonExecution {
    exit_code: i32,
    stdout: String,
    stderr: String,
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

    // WE JUST DO STRINGS FOR NOW BC WE DON'T KNOW IF IT'S NOTIFICATION OR REQUEST SO WE JUST
    // SERIALIZE THEM INTO STRINGS.
    // TODO: Check if it would just be better to use an unbounded_channel.
    let (tx, rx) = mpsc::channel::<String>(9999999999);

    register_worker(&mut reader, &mut writer, worker_id).await?;

    // Heartbeat task
    let heartbeat_tx = tx.clone();
    let heartbeat_state = Arc::clone(&worker_state);
    let heartbeat_task = tokio::spawn(heartbeat_clock(heartbeat_tx, heartbeat_state));

    // Writer task
    let writer_task = tokio::spawn(writer_loop(rx, writer));

    // Reader task
    let _reader_state = Arc::clone(&worker_state);
    let reader_task = tokio::spawn(reader_loop(reader, _reader_state));

    // Fetch task
    let fetcher_tx = tx.clone();
    let fetcher_state = Arc::clone(&worker_state);
    let fetcher_task = tokio::spawn(fetch_loop(fetcher_tx, fetcher_state, max_concurrent_tasks));

    // Executor task
    let executor_tx = tx.clone();
    let executor_state = Arc::clone(&worker_state);
    let executor_task = tokio::spawn(executor_loop(executor_tx, executor_state));

    tokio::select! {
        _ = writer_task => error!("[ERROR] Writer task crashed."),
        _ = reader_task => error!("[ERROR] Reader task crashed."),
        _ = fetcher_task => error!("[ERROR] Fetcher task crashed."),
        _ = heartbeat_task => error!("[ERROR] Heartbeat task crashed."),
        _ = executor_task => error!("[ERROR] Executor task crashed."),
    };
    Ok(())
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

async fn heartbeat_clock(heartbeat_tx: mpsc::Sender<String>, state: Arc<WorkerState>) {
    info!("[INFO] Starting heartbeat task");
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let heartbeat_notif = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: "heartbeat".into(),
            params: json!({
                "worker_id": state.id,
            }),
        };

        let msg = format_jrpc_message(heartbeat_notif).unwrap();

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
}

async fn writer_loop(mut rx: mpsc::Receiver<String>, mut writer: OwnedWriteHalf) {
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

async fn reader_loop(mut reader: BufReader<OwnedReadHalf>, state: Arc<WorkerState>) {
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

async fn fetch_loop(
    fetcher_tx: mpsc::Sender<String>,
    state: Arc<WorkerState>,
    max_concurrent_tasks: usize,
) {
    info!("[INFO] Starting fetching task");
    let mut interval = tokio::time::interval(Duration::from_secs(7));
    interval.tick().await;
    loop {
        interval.tick().await;
        if state.task_list.read().await.len() < max_concurrent_tasks {
            let fetch_task_msg = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: next_id().to_string(),
                method: "fetch_task".into(),
                params: json!({
                    "worker_id": state.id,
                }),
            };

            let msg = format_jrpc_message(fetch_task_msg).unwrap();

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
}

async fn executor_loop(writer_tx: mpsc::Sender<String>, state: Arc<WorkerState>) {
    info!("[INFO] Starting executor loop task");
    loop {
        if let Some(task) = state.task_list.write().await.pop_front() {
            info!("[INFO] Running task {}", task.id);
            let updated_task_status = TaskResult {
                id: task.id,
                name: task.name.clone(),
                code_b64: task.code_b64.clone(),
                result: None,
                status: TaskStatus::Running,
            };
            // TODO: Check these unwrap.
            let task_status = serde_json::to_value(&updated_task_status).unwrap();
            let updated_result = JsonRpcNotification {
                jsonrpc: "2.0".into(),
                method: "report_result".into(),
                params: task_status,
            };
            let message = format_jrpc_message(updated_result).unwrap();
            writer_tx.send(message).await.unwrap();

            let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
            let state_clone = Arc::clone(&state);

            let task_id = &task.id.clone();
            let handle = tokio::spawn(execute_task_select(
                writer_tx.clone(),
                state_clone,
                task,
                cancel_rx,
            ));
            state
                .running_tasks
                .write()
                .await
                .insert(*task_id, RunningTask { handle, cancel_tx });
        }
    }
}

async fn execute_task_select(
    writer_tx: mpsc::Sender<String>,
    state: Arc<WorkerState>,
    task: Task,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let code = match BASE64_STANDARD.decode(&task.code_b64) {
        Ok(c) => String::from_utf8_lossy(&c).to_string(),
        Err(_) => {
            let task_result = TaskResult {
                id: task.id,
                name: task.name.clone(),
                code_b64: task.code_b64.clone(),
                result: None,
                status: TaskStatus::Failed,
            };
            send_result_notification(&writer_tx, task_result).await;
            state.running_tasks.write().await.remove(&task.id);
            return;
        }
    };

    loop {
        let res = run_python_or_cancel(code.clone(), &mut cancel_rx).await;

        match res {
            // Ok(Some(status, stdout, stderr)) means successful execution.
            Ok(Some((status, stdout_buf, stderr_buf))) => {
                let exit_code = status.code().unwrap_or(-1);

                let python_result = PythonExecution {
                    exit_code,
                    stdout: String::from_utf8_lossy(&stdout_buf).to_string(),
                    stderr: String::from_utf8_lossy(&stderr_buf).to_string(),
                };
                let py_res_val = serde_json::to_value(&python_result).unwrap();

                if status.success() {
                    let task_result = TaskResult {
                        id: task.id,
                        name: task.name.clone(),
                        code_b64: task.code_b64.clone(),
                        result: Some(py_res_val),
                        status: TaskStatus::Completed,
                    };
                    send_result_notification(&writer_tx, task_result).await;
                    state.running_tasks.write().await.remove(&task.id);
                    info!("[INFO] Task {} completed with successful exit code.", &task.id);
                    break;
                } else {
                    info!(
                        "[INFO] Task {} failed (exit {}). Retrying...",
                        task.id, exit_code
                    );

                    let task_result = TaskResult {
                        id: task.id,
                        name: task.name.clone(),
                        code_b64: task.code_b64.clone(),
                        result: Some(py_res_val),
                        status: TaskStatus::Failed,
                    };
                    send_result_notification(&writer_tx, task_result).await;

                    // In between retries, wait a second and check if cancel signal has been
                    // received.
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(1)) => {
                            continue; // Retry loop
                        }
                        _ = &mut cancel_rx => {
                            // Cancelled while sleeping
                            handle_cancellation(&writer_tx, &state, &task).await;
                            break;
                        }
                    }
                }
            }
            // Ok(None) means cancellation got here, so we just break out of the loop.
            Ok(None) => {
                handle_cancellation(&writer_tx, &state, &task).await;
                break;
            }
            // Err(_) means some error happened even before the execution, so we just try
            // again.
            Err(_) => {
                continue;
            }
        }
    }
}

/// Returns Ok(Some((exit_status, stdout, stderr))) if successful, Ok(None) if cancelled, and Err(_) if failed and needs
/// to retry.
async fn run_python_or_cancel(
    code: String,
    cancel_rx: &mut oneshot::Receiver<()>,
) -> anyhow::Result<Option<(ExitStatus, Vec<u8>, Vec<u8>)>> {
    let mut child = match Command::new("uv")
        .arg("run")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => anyhow::bail!("Could not create child process."),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if (stdin.write_all(code.as_bytes()).await).is_err() {
            anyhow::bail!("Could not write code to UV stdin.");
        }
        // Drop stdin to let know its EOF.
        drop(child.stdin.take());
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if stdout.is_none() || stderr.is_none() {
        anyhow::bail!("Could not take stdout or stderr from uv process.");
    }

    // SAFETY: We literally checked before and bail out if they are none.
    let mut stdout = stdout.unwrap();
    let mut stderr = stderr.unwrap();

    tokio::select! {
        status_res = child.wait() => {
            match status_res {
                Ok(status) => {
                    let mut stdout_buf = Vec::new();
                    let mut stderr_buf = Vec::new();
                    // Read output after wait (Note: for large output, read concurrently to avoid deadlocks)
                    let _ = tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut stdout_buf).await;
                    let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_buf).await;
                    Ok(Some((status, stdout_buf, stderr_buf)))
                }
                Err(_) => anyhow::bail!("An error happened when waiting the process"),
            }
        }
        _ = cancel_rx => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Ok(None) // Ok(None) because cancellation got in before.
        }
    }
}

async fn send_result_notification(writer_tx: &mpsc::Sender<String>, result: TaskResult) {
    let val = serde_json::to_value(result).unwrap();
    let notification = JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: "report_result".into(),
        params: val,
    };
    let msg = format_jrpc_message(notification).unwrap();
    let _ = writer_tx.send(msg).await;
}

async fn handle_cancellation(
    writer_tx: &mpsc::Sender<String>,
    state: &Arc<WorkerState>,
    task: &Task,
) {
    warn!("[WARNING] Task {} has been cancelled due to too many attempts.", task.id);
    state.running_tasks.write().await.remove(&task.id);
    let task_result = TaskResult {
        id: task.id,
        name: task.name.clone(),
        code_b64: task.code_b64.clone(),
        result: None,
        status: TaskStatus::Cancelled,
    };
    send_result_notification(writer_tx, task_result).await;
}

#[derive(Serialize, Deserialize, Debug)]
struct StopExecutionNotificationParams {
    task_id: Uuid,
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
                    info!("[INFO] Cancelled task {}", &params.task_id);
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
