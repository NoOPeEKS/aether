use std::sync::Once;

use aether_broker::DefaultBroker;
use aether_core::broker::storage::InMemoryStorage;
use aether_core::capabilities::{CPUArchitecture, WorkerCapabilities};
use aether_core::traits::Broker;
use aether_worker::Worker;
static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

fn get_broker() -> DefaultBroker<InMemoryStorage> {
    let storage = InMemoryStorage::new();
    DefaultBroker::new(storage)
}

fn get_worker() -> Worker {
    let capabilities = WorkerCapabilities {
        gpu: false,
        arch: CPUArchitecture::X86_64,
    };
    Worker::new("test-worker", "0.0.0.0:8081", 10, capabilities)
}

#[tokio::test]
async fn will_fetch_if_no_capabilities_specified() {
    init_tracing();
    let broker = get_broker();
    tokio::spawn(async move {
        broker
            .run(8080, 8081)
            .await
            .expect("Broker should not crash");
    });
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
    let worker = get_worker();
    tokio::spawn(async move {
        worker.run().await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}

#[tokio::test]
async fn will_fetch_if_compatible_caps() {
    init_tracing();
    let broker = get_broker();
    tokio::spawn(async move {
        broker
            .run(8080, 8081)
            .await
            .expect("Broker should not crash");
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // The base64 is from this code:
        // for i in range(0, 5):
        //     print(f"Valor i: {i}")
        .body("{\"name\":\"sample-task\", \"code_b64\": \"Zm9yIGkgaW4gcmFuZ2UoMCwgNSk6CiAgICBwcmludChmIlZhbG9yIGk6IHtpfSIpCg==\", \"priority\": \"high\", \"capabilities\": {\"gpu\": false, \"arch\": \"x86_64\"}}")
        .send().await.unwrap();
    let worker = get_worker();
    tokio::spawn(async move {
        worker.run().await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}

#[tokio::test]
async fn will_fetch_if_any_arch() {
    init_tracing();
    let broker = get_broker();
    tokio::spawn(async move {
        broker
            .run(8080, 8081)
            .await
            .expect("Broker should not crash");
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // The base64 is from this code:
        // for i in range(0, 5):
        //     print(f"Valor i: {i}")
        .body("{\"name\":\"sample-task\", \"code_b64\": \"Zm9yIGkgaW4gcmFuZ2UoMCwgNSk6CiAgICBwcmludChmIlZhbG9yIGk6IHtpfSIpCg==\", \"priority\": \"high\", \"capabilities\": {\"gpu\": false, \"arch\": \"any\"}}")
        .send().await.unwrap();
    let worker = get_worker();
    tokio::spawn(async move {
        worker.run().await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}

#[tokio::test]
async fn wont_fetch_if_not_compatible_capabilities() {
    init_tracing();
    let broker = get_broker();
    tokio::spawn(async move {
        broker
            .run(8080, 8081)
            .await
            .expect("Broker should not crash");
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    let client = reqwest::Client::new();
    _ = client
        .post("http://localhost:8080/api/v1/tasks")
        .header("Content-Type", "application/json")
        // The base64 is from this code:
        // for i in range(0, 5):
        //     print(f"Valor i: {i}")
        .body("{\"name\":\"sample-task\", \"code_b64\": \"Zm9yIGkgaW4gcmFuZ2UoMCwgNSk6CiAgICBwcmludChmIlZhbG9yIGk6IHtpfSIpCg==\", \"priority\": \"high\", \"capabilities\": {\"gpu\": true, \"arch\": \"x86_64\"}}")
        .send().await.unwrap();
    let worker = get_worker();
    tokio::spawn(async move {
        worker.run().await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}
