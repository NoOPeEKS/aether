use std::path::Path;

use aether_core::capabilities::CPUArchitecture;
use aether_core::http::{
    CancelTaskResponse, CreateTaskResponse, GetAllTasksResponse, GetTaskResponse,
};
use aether_core::task::TaskPriority;
use base64::prelude::*;
use reqwest::StatusCode;
use serde_json::json;

use crate::commands::{SupportedArchs, SupportedPriorities};
use crate::error::CliError;

pub fn parse_task_file(file_path: &str) -> Result<String, CliError> {
    let path = Path::new(file_path)
        .canonicalize()
        .map_err(|_| CliError::ParseTask("Could not canonicalize provided path.".into()))?;

    let path_exists = path
        .try_exists()
        .map_err(|_| CliError::ParseTask("Path does not exist.".into()))?;

    if !path_exists {
        return Err(CliError::ParseTask(
            "The provided file path must exist!".into(),
        ));
    }
    if let Some(ext) = path.extension()
        && ext != "py"
    {
        return Err(CliError::ParseTask(
            "Task file must be a python file!".into(),
        ));
    }

    let file_contents = std::fs::read_to_string(path)
        .map_err(|_| CliError::ParseTask("Could not read file contents.".into()))?;
    let encoded = BASE64_STANDARD.encode(file_contents.as_bytes());
    Ok(encoded)
}

#[allow(clippy::too_many_arguments)]
pub async fn send_task_to_broker(
    broker_ip: &str,
    broker_api_port: usize,
    task_b64: &str,
    name: &str,
    priority: SupportedPriorities,
    gpu: bool,
    arch: SupportedArchs,
    token: &str,
) -> Result<CreateTaskResponse, CliError> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/tasks");
    let priority: TaskPriority = priority.into();
    let arch: CPUArchitecture = arch.into();
    let body = json!({
        "name": name,
        "code_b64": task_b64,
        "priority": priority,
        "capabilities": {
            "gpu": gpu,
            "arch": arch,
        },
    });
    let bearer = format!("Bearer {token}");
    let response = client
        .post(broker_addr)
        .header("Content-Type", "application/json")
        .header("Authorization", bearer)
        .body(body.to_string())
        .send()
        .await
        .map_err(|_| CliError::SendRequest)?;
    let status = response.status();
    if status != StatusCode::CREATED {
        return Err(CliError::UnexpectedStatusCode(status));
    }
    let resp_body = response
        .json::<CreateTaskResponse>()
        .await
        .map_err(|_| CliError::DeserializeRequest)?;
    Ok(resp_body)
}

pub async fn check_task(
    broker_ip: &str,
    broker_api_port: usize,
    task_id: &str,
    token: &str,
) -> Result<GetTaskResponse, CliError> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/tasks/{task_id}");
    let bearer = format!("Bearer {token}");
    let response = client
        .get(broker_addr)
        .header("Authorization", bearer)
        .send()
        .await
        .map_err(|_| CliError::SendRequest)?;
    let status = response.status();
    if status != StatusCode::OK {
        return Err(CliError::UnexpectedStatusCode(status));
    }
    let resp_body = response
        .json::<GetTaskResponse>()
        .await
        .map_err(|_| CliError::DeserializeRequest)?;
    Ok(resp_body)
}

pub async fn list_tasks(
    broker_ip: &str,
    broker_api_port: usize,
    token: &str,
) -> Result<GetAllTasksResponse, CliError> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/tasks");
    let bearer = format!("Bearer {token}");
    let response = client
        .get(broker_addr)
        .header("Authorization", bearer)
        .send()
        .await
        .map_err(|_| CliError::SendRequest)?;
    let status = response.status();
    if status != StatusCode::OK {
        return Err(CliError::UnexpectedStatusCode(status));
    }
    let resp_body = response
        .json::<GetAllTasksResponse>()
        .await
        .map_err(|_| CliError::DeserializeRequest)?;
    Ok(resp_body)
}

pub async fn cancel_task(
    broker_ip: &str,
    broker_api_port: usize,
    task_id: &str,
    token: &str,
) -> Result<CancelTaskResponse, CliError> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/tasks/{task_id}/cancel");
    let bearer = format!("Bearer {token}");
    let response = client
        .post(broker_addr)
        .header("Authorization", bearer)
        .send()
        .await
        .map_err(|_| CliError::SendRequest)?;
    if response.status() != StatusCode::OK {
        return Err(CliError::UnexpectedStatusCode(response.status()));
    }
    let resp_body = response
        .json::<CancelTaskResponse>()
        .await
        .map_err(|_| CliError::DeserializeRequest)?;
    Ok(resp_body)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn non_existant_file_returns_err() {
        assert!(parse_task_file("nonexistant.py").is_err())
    }

    #[test]
    fn non_python_file_returns_err() {
        assert!(parse_task_file("./Cargo.toml").is_err())
    }

    #[test]
    fn encode_correct_file_in_base_64() {
        let mut file = tempfile::NamedTempFile::with_prefix("py").unwrap();
        writeln!(file, "import time; time.sleep(30)").unwrap();

        let path = file.path().to_str().unwrap();

        let res = parse_task_file(path);

        assert!(res.is_ok())
    }
}
