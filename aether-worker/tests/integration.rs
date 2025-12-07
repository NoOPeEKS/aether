use std::sync::Once;
static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

#[tokio::test]
async fn correct_execution() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // The base64 is from this code:
        // for i in range(0, 5):
        //     print(f"Valor i: {i}")
        .body("{\"name\":\"sample-task\", \"code_b64\": \"Zm9yIGkgaW4gcmFuZ2UoMCwgNSk6CiAgICBwcmludChmIlZhbG9yIGk6IHtpfSIpCg==\", \"priority\": \"high\"}")
        .send().await.unwrap();
    tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}

#[tokio::test]
async fn retry_and_cancel_incorrect_execution() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // The base64 is a wrong syntax python script, so it will fail and retry many times until
        // cancelled for too many attempts by broker.
        .body("{\"name\":\"sample-task\", \"code_b64\": \"aW1wb3J0IG9zCgplcnJvcm9oZXJlCg==\", \"priority\": \"high\"}")
        .send().await.unwrap();
    tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}
