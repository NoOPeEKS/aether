use std::process::{ExitStatus, Stdio};
use std::sync::Arc;

use aether_common::jrpc::{JsonRpcNotification, format_jrpc_message};
use aether_common::task::{Task, TaskResult, TaskStatus};
use base64::prelude::*;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use tracing::{info, warn};

use crate::state::{PythonExecution, RunningTask, WorkerState};

pub async fn executor_loop(writer_tx: mpsc::Sender<String>, state: Arc<WorkerState>) {
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
                    info!(
                        "[INFO] Task {} completed with successful exit code.",
                        &task.id
                    );
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
    warn!("[WARNING] Task {} has been cancelled", task.id);
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
