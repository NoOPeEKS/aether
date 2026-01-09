use std::path::Path;

use aether_core::capabilities::CPUArchitecture;
use aether_core::http::{CreateTaskResponse, GetTaskResponse};
use aether_core::task::{TaskPriority, TaskResult, TaskStatus};
use base64::prelude::*;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::commands::{SupportedArchs, SupportedPriorities};

pub fn parse_task_file(file_path: &str) -> anyhow::Result<String> {
    let path = Path::new(file_path).canonicalize()?;
    let path_exists = path.try_exists()?;

    if !path_exists {
        anyhow::bail!("The provided file path must exist!");
    }
    if let Some(ext) = path.extension()
        && ext != "py"
    {
        anyhow::bail!("Task file must be a python file!");
    }

    let file_contents = std::fs::read_to_string(path)?;
    let encoded = BASE64_STANDARD.encode(file_contents.as_bytes());
    Ok(encoded)
}

pub async fn send_task_to_broker(
    broker_ip: &str,
    broker_api_port: usize,
    task_b64: &str,
    name: &str,
    priority: SupportedPriorities,
    gpu: bool,
    arch: SupportedArchs,
) -> anyhow::Result<CreateTaskResponse> {
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
    let response = client
        .post(broker_addr)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let status = response.status();
    if status != StatusCode::CREATED {
        anyhow::bail!("Got an unexpected Status Code ({status:?}).");
    }
    let resp_body = response.json::<CreateTaskResponse>().await?;
    Ok(resp_body)
}

pub async fn check_task(
    broker_ip: &str,
    broker_api_port: usize,
    task_id: &str,
) -> anyhow::Result<GetTaskResponse> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/tasks/{task_id}");
    let response = client.get(broker_addr).send().await?;
    let resp_body = response.json::<GetTaskResponse>().await?;
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
