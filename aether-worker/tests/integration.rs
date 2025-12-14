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

#[tokio::test]
async fn broker_cancels_too_long_task() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // THe base64 is code for "import time; time.sleep(40)", which should take longer than
        // default 30 secs and make the broker autocancel the task.
        .body("{\"name\":\"sample-task\", \"code_b64\": \"aW1wb3J0IHRpbWU7IHRpbWUuc2xlZXAoNDApCg==\", \"priority\": \"high\"}")
        .send().await.unwrap();
    tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::time::sleep(tokio::time::Duration::from_secs(80)).await;
}

#[tokio::test]
async fn multiple_simultaneous_correct_tasks() {
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
async fn multiple_simultaneous_tasks_correct_and_incorrect() {
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

#[tokio::test]
async fn simultaneous_tasks_long_and_incorrect() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // THe base64 is code for "import time; time.sleep(40)", which should take longer than
        // default 30 secs and make the broker autocancel the task.
        .body("{\"name\":\"sample-task\", \"code_b64\": \"aW1wb3J0IHRpbWU7IHRpbWUuc2xlZXAoNDApCg==\", \"priority\": \"high\"}")
        .send().await.unwrap();
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

#[tokio::test]
async fn worker_reconnection() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let mut worker_handle =
        tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::select! {
        _ = &mut worker_handle => {}
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
            worker_handle.abort();
        }
    };
    // Wait for old worker to completely die.
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
}
