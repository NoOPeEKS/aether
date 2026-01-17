use std::sync::Arc;

use aether_core::auth::{Permission, User};
use aether_core::http::{
    CancelTaskResponse, CreateTaskRequest, CreateTaskResponse, GetAllTasksResponse, GetTaskResponse,
};
use aether_core::jrpc::{JsonRpcNotification, format_jrpc_message};
use aether_core::task::{Task, TaskStatus};
use aether_core::traits::Storage;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use tracing::info;
use uuid::Uuid;

use crate::jrpc::params::StopExecutionNotificationParams;
use crate::state::BrokerState;

pub async fn create_task_handler<S: Storage>(
    Extension(user): Extension<User>,
    State(state): State<Arc<BrokerState<S>>>,
    Json(task): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    info!("[INFO] A task has been requested at the POST /tasks");

    if !user.is_admin
        && !user.permissions.contains(&Permission::CreateTask)
        && !user.permissions.contains(&Permission::All)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let id = Uuid::new_v4();
    let new_task = Task {
        id,
        owner_id: user.id,
        name: task.name,
        code_b64: task.code_b64,
        priority: task.priority,
        capabilities: task.capabilities,
    };

    if state.storage.enqueue_task(new_task).await.is_ok() {
        (
            StatusCode::CREATED,
            Json(CreateTaskResponse::Ok {
                task_id: id,
                status: TaskStatus::Queued,
            }),
        )
            .into_response()
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CreateTaskResponse::Error {
                message: String::from("Could not create task successfully."),
            }),
        )
            .into_response()
    }
}

pub async fn get_task_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Extension(user): Extension<User>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    if !user.is_admin
        && !user.permissions.contains(&Permission::CheckTask)
        && !user.permissions.contains(&Permission::All)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if let Some(task) = state.storage.get_task_result(task_id).await {
        match task.status {
            TaskStatus::Completed | TaskStatus::Queued | TaskStatus::Running => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: None,
                }),
            )
                .into_response(),
            TaskStatus::Failed => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: Some("An error occured parsing inputs.".to_string()),
                }),
            )
                .into_response(),
            TaskStatus::Cancelled => (
                StatusCode::OK,
                Json(GetTaskResponse {
                    task: Some(task),
                    error: Some("This task got cancelled due too many attempts".to_string()),
                }),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(GetTaskResponse {
                task: None,
                error: Some("No task was found with the provided id".to_string()),
            }),
        )
            .into_response()
    }
}

pub async fn get_all_tasks_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    if !user.is_admin
        && !user.permissions.contains(&Permission::ListTasks)
        && !user.permissions.contains(&Permission::All)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if let Some(tasks) = state.storage.get_all_tasks().await {
        (
            StatusCode::OK,
            Json(GetAllTasksResponse { tasks: Some(tasks) }),
        )
            .into_response()
    } else {
        (StatusCode::OK, Json(GetAllTasksResponse { tasks: None })).into_response()
    }
}

fn format_cancel_response(
    status_code: StatusCode,
    message: &str,
) -> (StatusCode, Json<CancelTaskResponse>) {
    (
        status_code,
        Json(CancelTaskResponse {
            message: message.into(),
        }),
    )
}

pub async fn cancel_task_handler<S: Storage>(
    State(state): State<Arc<BrokerState<S>>>,
    Extension(user): Extension<User>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    if !user.is_admin
        && !user.permissions.contains(&Permission::CancelTask)
        && !user.permissions.contains(&Permission::All)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match serde_json::to_value(StopExecutionNotificationParams { task_id }) {
        Ok(stop_exec_params) => {
            if let Some(tr) = state.storage.get_task_result(task_id).await {
                if tr.owner_id != user.id || !user.is_admin {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                match tr.status {
                    TaskStatus::Completed => {
                        return format_cancel_response(
                            StatusCode::CONFLICT,
                            "Cannot cancel an already completed task.",
                        )
                        .into_response();
                    }
                    TaskStatus::Cancelled => {
                        return format_cancel_response(
                            StatusCode::OK,
                            "Task was already cancelled.",
                        )
                        .into_response();
                    }
                    _ => {
                        // If its on any state that is not completed or cancelled, means we have a
                        // lease up and we can know which worker is running it.
                        let lease = state.storage.get_lease(&task_id).await;
                        if let Some(lease) = lease
                            && let Some(wsession) =
                                state.worker_sessions.read().await.get(&lease.worker_id)
                        {
                            let stop_notif = JsonRpcNotification {
                                jsonrpc: "2.0".into(),
                                method: "stop_execution".into(),
                                params: stop_exec_params,
                            };
                            if let Ok(str_msg) = format_jrpc_message(stop_notif) {
                                if wsession.sender.send(str_msg).is_ok() {
                                    return format_cancel_response(
                                        StatusCode::OK,
                                        format!("Task {task_id} cancelled successfully.").as_ref(),
                                    )
                                    .into_response();
                                }
                                return format_cancel_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to send stop_execution_notification",
                                )
                                .into_response();
                            }
                            return format_cancel_response(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Could not serialize stop execution notification",
                            )
                            .into_response();
                        }
                        return format_cancel_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Could not retrieve the task lease.",
                        )
                        .into_response();
                    }
                }
            }
            format_cancel_response(StatusCode::NOT_FOUND, "The solicited task does not exist.")
                .into_response()
        }
        Err(_) => format_cancel_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not serialize stop execution params correctly",
        )
        .into_response(),
    }
}
