use aether_core::jrpc::{
    JsonRpcError, JsonRpcErrorCode, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    format_jrpc_message,
};
use aether_core::task::{Task, TaskPriority, TaskResult, TaskStatus};
use aether_core::traits::Storage;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

use crate::jrpc::params::*;
use crate::state::{BrokerState, WorkerInfo, WorkerSession};

const MAX_ATTEMPTS: usize = 5;

pub async fn process_jsonrpc_message<S: Storage>(
    message: &[u8],
    state: &BrokerState<S>,
    worker_sender: UnboundedSender<String>,
) -> anyhow::Result<Option<JsonRpcResponse>> {
    let message = Message::from_slice(message);
    if let Some(Message::Request(request)) = message {
        let request_method = RequestMethod::from(&request.method as &str);
        info!("[INFO] Received a request of type {:?}", request_method);
        match request_method {
            RequestMethod::RegisterWorker => {
                match register_worker(state, request.clone(), worker_sender).await {
                    Ok(resp) => return Ok(Some(resp)),
                    Err(_) => {
                        return Ok(Some(JsonRpcResponse {
                            jsonrpc: "2.0".into(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: JsonRpcErrorCode::InvalidRequest,
                                message: "Something occured trying to parse the request.".into(),
                                data: None,
                            }),
                        }));
                    }
                }
            }
            RequestMethod::FetchTask => match fetch_task_for_worker(state, request.clone()).await {
                Ok(resp) => return Ok(Some(resp)),
                Err(_) => {
                    return Ok(Some(JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: JsonRpcErrorCode::InternalError,
                            message: "An error occurred internally trying to fetch a task".into(),
                            data: None,
                        }),
                    }));
                }
            },
            RequestMethod::Unknown(method) => {
                return Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: JsonRpcErrorCode::MethodNotFound,
                        message: format!("Method '{method}' not found or not allowed"),
                        data: None,
                    }),
                }));
            }
        }
    } else if let Some(Message::Notification(notification)) = message {
        let notification_method = NotificationMethod::from(&notification.method as &str);
        info!(
            "[INFO] Received a request of type {:?}",
            notification_method
        );
        match notification_method {
            NotificationMethod::Heartbeat => {
                process_heartbeat(state, notification).await;
                return Ok(None);
            }
            NotificationMethod::ReportResult => {
                match handle_report_result(state, notification).await {
                    Ok(_) => {}
                    Err(_) => warn!(
                        "[WARNING] An error occured while processing a 'report_result' notification."
                    ),
                }
                return Ok(None);
            }
            NotificationMethod::WorkerShutdown => {
                handle_worker_shutdown(state, notification).await;
                return Ok(None);
            }
            NotificationMethod::Unknown(_) => return Ok(None),
        }
    }
    Ok(None)
}

async fn register_worker<S: Storage>(
    state: &BrokerState<S>,
    request: JsonRpcRequest,
    worker_sender: UnboundedSender<String>,
) -> anyhow::Result<JsonRpcResponse> {
    let register_req: RegisterWorkerRequestParams = serde_json::from_value(request.params)?;
    let mut workers = state.worker_registry.write().await;
    let mut sessions = state.worker_sessions.write().await;

    match workers.get_mut(&register_req.worker_id) {
        // Worker already existed, recreate session.
        Some(winfo) => {
            info!("[INFO] Worker {} reconnected", &register_req.worker_id);
            let now = tokio::time::Instant::now();
            winfo.last_heartbeat = now;
            winfo.active = true;
            sessions.insert(
                register_req.worker_id.clone(),
                WorkerSession {
                    sender: worker_sender,
                    connected_at: now,
                },
            );
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(serde_json::to_value(RegisterWorkerResponseParams {
                    status: "registered".into(),
                })?),
                error: None,
            })
        }
        // New worker, create both register and session.
        None => {
            info!(
                "[INFO] Registering new worker with id = {}",
                &register_req.worker_id
            );
            workers.insert(
                register_req.worker_id.clone(),
                WorkerInfo {
                    worker_id: register_req.worker_id.clone(),
                    last_heartbeat: tokio::time::Instant::now(),
                    active: true,
                },
            );
            sessions.insert(
                register_req.worker_id,
                WorkerSession {
                    sender: worker_sender,
                    connected_at: tokio::time::Instant::now(),
                },
            );
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(serde_json::to_value(RegisterWorkerResponseParams {
                    status: "registered".into(),
                })?),
                error: None,
            })
        }
    }
}

async fn fetch_task_for_worker<S: Storage>(
    state: &BrokerState<S>,
    request: JsonRpcRequest,
) -> anyhow::Result<JsonRpcResponse> {
    let req_params: FetchTaskRequestParams = serde_json::from_value(request.params)?;

    let error_response = |message: &str| -> anyhow::Result<JsonRpcResponse> {
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: request.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: JsonRpcErrorCode::InvalidRequest,
                message: message.into(),
                data: None,
            }),
        })
    };

    let success_response = |task: FetchTaskResponseResult| -> anyhow::Result<JsonRpcResponse> {
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: request.id.clone(),
            result: Some(serde_json::to_value(task)?),
            error: None,
        })
    };

    let workers = state.worker_registry.read().await;
    if !workers.contains_key(&req_params.worker_id) {
        info!("[INFO] Could not fetch task from a non-registered worker.");
        return error_response("Cannot fetch task from non-registered worker.");
    }

    if let Some(winfo) = workers.get(&req_params.worker_id)
        && !winfo.active
    {
        info!("[INFO] Could not fetch task from an inactive worker.");
        return error_response("Cannot fetch task from an inactive worker");
    }

    if let Some(task) = state.storage.dequeue_task(&req_params.worker_id).await {
        info!("[INFO] Sending task to ID = {}", &req_params.worker_id);
        success_response(FetchTaskResponseResult { task: Some(task) })
    } else {
        info!("[INFO] Sending None task to ID = {}", &req_params.worker_id);
        success_response(FetchTaskResponseResult { task: None })
    }
}

async fn process_heartbeat<S: Storage>(state: &BrokerState<S>, notification: JsonRpcNotification) {
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
}

async fn handle_report_result<S: Storage>(
    state: &BrokerState<S>,
    notification: JsonRpcNotification,
) -> anyhow::Result<()> {
    if let Ok(task_result) = serde_json::from_value::<TaskResult>(notification.params)
        && state.storage.contains_result(task_result.id).await
    {
        info!("[INFO] Result from task ID = {} received.", task_result.id);
        state
            .storage
            .store_result(task_result.id, task_result.clone())
            .await;
        if task_result.status == TaskStatus::Completed {
            state.storage.remove_lease(&task_result.id).await;
        } else if task_result.status == TaskStatus::Failed {
            let (too_many_attempts, worker_id) = state
                .storage
                .mark_task_failed(&task_result.id, MAX_ATTEMPTS)
                .await?;

            if too_many_attempts
                && let Some(_) = state.worker_registry.read().await.get(&worker_id)
                && let Some(session) = state.worker_sessions.read().await.get(&worker_id)
            {
                let notif = JsonRpcNotification {
                    jsonrpc: "2.0".into(),
                    method: "stop_execution".into(),
                    params: serde_json::to_value(StopExecutionNotificationParams {
                        task_id: task_result.id,
                    })?,
                };
                // TODO: Handle this better.
                session.sender.send(format_jrpc_message(notif)?)?;
            }
        } else if task_result.status == TaskStatus::Cancelled {
            state.storage.remove_lease(&task_result.id).await;
        }
    }
    Ok(())
}

async fn handle_worker_shutdown<S: Storage>(
    state: &BrokerState<S>,
    notification: JsonRpcNotification,
) {
    if let Ok(notif_params) =
        serde_json::from_value::<WorkerShutdownNotificationParams>(notification.params)
    {
        if state
            .worker_sessions
            .write()
            .await
            .remove(&notif_params.worker_id)
            .is_none()
        {
            // We return early and do nothing because it's supposed to have a WorkerSession
            // to be able to send shutdown.
            return;
        }

        if state
            .worker_registry
            .write()
            .await
            .remove(&notif_params.worker_id)
            .is_none()
        {
            // We return early and do nothing because it's supposed to have a WorkerInfo
            // registered to be able to send shutdown.
            return;
        }

        let ids = state
            .storage
            .remove_leases_of_worker(&notif_params.worker_id)
            .await;
        if let Ok(ids) = ids {
            for id in ids.into_iter() {
                let task_result = state.storage.get_task_result(id).await;
                if let Some(mut res) = task_result
                    && (res.status == TaskStatus::Running || res.status == TaskStatus::Queued)
                {
                    // We set prio to high because this was already being executed before shutdown.
                    let new_task = Task {
                        id: res.id,
                        name: res.name.clone(),
                        code_b64: res.code_b64.clone(),
                        priority: TaskPriority::High,
                    };
                    res.status = TaskStatus::Cancelled;
                    state.storage.enqueue_task(new_task).await;
                    state.storage.store_result(res.id, res).await;
                }
            }
        }
    }
}

enum Message {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

impl Message {
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        let msg_val: serde_json::Value = serde_json::from_slice(slice).ok()?;
        if msg_val.get("id").is_some() {
            // It's a request
            let request: JsonRpcRequest = serde_json::from_value(msg_val).ok()?;
            Some(Self::Request(request))
        } else {
            // It's a notification
            let notification: JsonRpcNotification = serde_json::from_value(msg_val).ok()?;
            Some(Self::Notification(notification))
        }
    }
}

#[derive(Debug)]
enum RequestMethod<'a> {
    RegisterWorker,
    FetchTask,
    Unknown(&'a str),
}

#[derive(Debug)]
enum NotificationMethod {
    Heartbeat,
    ReportResult,
    WorkerShutdown,
    Unknown(()),
}

impl<'a> From<&'a str> for RequestMethod<'a> {
    fn from(value: &'a str) -> Self {
        match value {
            "register_worker" => Self::RegisterWorker,
            "fetch_task" => Self::FetchTask,
            val => Self::Unknown(val),
        }
    }
}

impl From<&str> for NotificationMethod {
    fn from(value: &str) -> Self {
        match value {
            "heartbeat" => Self::Heartbeat,
            "report_result" => Self::ReportResult,
            "worker_shutdown" => Self::WorkerShutdown,
            _ => Self::Unknown(()),
        }
    }
}
