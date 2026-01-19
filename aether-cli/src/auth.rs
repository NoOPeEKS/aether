use aether_core::http::{LoginRequest, LoginResponse};

use crate::error::CliError;

pub async fn get_login_jwt(
    broker_ip: &str,
    broker_api_port: usize,
    username: &str,
    password: &str,
) -> Result<LoginResponse, CliError> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/auth/login");
    let req_body = LoginRequest {
        username: username.into(),
        password: password.into(),
    };
    let req_body = serde_json::to_string(&req_body).map_err(|_| CliError::SerializeSerdeError)?;
    let resp = client
        .post(broker_addr)
        .header("Content-Type", "application/json")
        .body(req_body)
        .send()
        .await
        .map_err(|_| CliError::SendRequestError)?;
    let resp_body = resp
        .json::<LoginResponse>()
        .await
        .map_err(|_| CliError::DeserializeRequestError)?;
    Ok(resp_body)
}
