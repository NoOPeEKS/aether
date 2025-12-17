use std::sync::Once;

use aether_broker::DefaultBroker;
use aether_core::traits::Broker;
use aether_core::broker::storage::InMemoryStorage;
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
    Worker::new("test-worker", "0.0.0.0:8081", 10)
}

#[tokio::test]
async fn cancellation_signal() {
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
        // THe base64 is code for "import time; time.sleep(40)", which should take longer than
        // default 30 secs and make the broker autocancel the task.
        .body("{\"name\":\"sample-task\", \"code_b64\": \"aW1wb3J0IHRpbWU7IHRpbWUuc2xlZXAoNDApCg==\", \"priority\": \"high\"}")
        .send().await.unwrap();
    let worker = get_worker();
    let cancellation = worker.shutdown_token.clone();
    tokio::spawn(async move {
        worker.run().await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    cancellation.cancel();
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
}
