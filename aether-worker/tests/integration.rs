use std::sync::Once;
static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

#[tokio::test]
async fn integration_test() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        .body("{\"name\":\"sample-task\", \"code_b64\": \"dGhpcyBpcyBhIHNhbXBsZSBjb2RlCg==\"}")
        .send().await.unwrap();
    // TODO: Check why it's correctly sent and created, but the worker can't seem to be able to
    // fetch it.
    tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}
