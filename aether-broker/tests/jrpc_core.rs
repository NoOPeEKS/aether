use std::sync::{Arc, Once};
use std::time::Duration;

use aether_broker::jrpc::server::create_jrpc_server;
use aether_broker::state::BrokerState;
use aether_core::broker::storage::InMemoryStorage;
static INIT: Once = Once::new();

mod test_utils;
use test_utils::{TestWorker, TestWorkerWorkflow, WorkerEvent, jrpc::get_random_available_port};

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

#[tokio::test]
async fn test_worker_register() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::RegisterOnly);
    let (events, handle) = tw.run(port).await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    handle.abort();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
}
