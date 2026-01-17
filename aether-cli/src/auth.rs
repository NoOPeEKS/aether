use aether_core::http::{LoginRequest, LoginResponse};

pub async fn get_login_jwt(
    broker_ip: &str,
    broker_api_port: usize,
    username: &str,
    password: &str,
) -> anyhow::Result<LoginResponse> {
    let client = reqwest::Client::new();
    let broker_addr = format!("http://{broker_ip}:{broker_api_port}/api/v1/auth/login");
    let req_body = LoginRequest {
        username: username.into(),
        password: password.into(),
    };
    let req_body = serde_json::to_string(&req_body)?;
    let resp = client
        .post(broker_addr)
        .header("Content-Type", "application/json")
        .body(req_body)
        .send()
        .await?;
    let resp_body = resp.json::<LoginResponse>().await?;
    Ok(resp_body)
}
