use std::sync::{Arc, Once};
use std::time::Duration;

use aether_broker::jrpc::server::create_jrpc_server;
use aether_broker::state::BrokerState;
use aether_core::broker::storage::InMemoryStorage;
use aether_core::task::{Task, TaskPriority, TaskResult, TaskStatus};
use uuid::Uuid;
static INIT: Once = Once::new();

mod test_utils;
use aether_core::traits::Storage;
use test_utils::jrpc::get_random_available_port;
use test_utils::{TestWorker, TestWorkerWorkflow, WorkerAction, WorkerEvent};

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

async fn create_test_task(name: &str, priority: TaskPriority) -> Task {
    Task {
        id: Uuid::new_v4(),
        owner_id: Uuid::new_v4(),
        name: name.to_string(),
        code_b64: "cHJpbnQoIkhlbGxvIFdvcmxkIik=".to_string(), // base64 for print("Hello World")
        priority,
        capabilities: None,
    }
}

#[tokio::test]
async fn test_fetch_task_no_tasks() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Fetch,
    ]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events.contains(&WorkerEvent::SentFetch));
    assert!(events.contains(&WorkerEvent::ReceivedNoTask));
}

#[tokio::test]
async fn test_fetch_task_with_task() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let task = create_test_task("test_task", TaskPriority::High).await;
    storage.enqueue_task(task).await.unwrap();
    let state = Arc::new(BrokerState::new(storage));
    let state_cl = Arc::clone(&state);
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Fetch,
    ]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events.contains(&WorkerEvent::SentFetch));
    assert!(events.contains(&WorkerEvent::ReceivedTask));
    assert_eq!(state_cl.storage.high_prio.read().await.len(), 0);
}

#[tokio::test]
async fn test_heartbeat_active() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Heartbeat,
        WorkerAction::Fetch,
    ]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events.contains(&WorkerEvent::SentHeartbeat));
    assert!(events.contains(&WorkerEvent::SentFetch));
    assert!(events.contains(&WorkerEvent::ReceivedNoTask));
}

#[tokio::test]
async fn test_heartbeat_inactive() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![WorkerAction::Register]));
    let (events1, handle1) = tw.run(port).await;
    handle1.await.unwrap();

    // Wait for timeout
    tokio::time::sleep(Duration::from_secs(16)).await;

    let tw2 = TestWorker::new(TestWorkerWorkflow::Custom(vec![WorkerAction::Fetch]));
    let (events2, handle2) = tw2.run(port).await;
    handle2.await.unwrap();

    let events2 = events2.lock().unwrap();
    assert!(events2.contains(&WorkerEvent::SentFetch));
    assert!(events2.contains(&WorkerEvent::FetchError));
}

#[tokio::test]
async fn test_report_result_success() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let task = create_test_task("test_task", TaskPriority::High).await;
    storage.enqueue_task(task.clone()).await.unwrap();
    let result = TaskResult {
        id: task.id,
        owner_id: task.owner_id,
        name: task.name.clone(),
        code_b64: task.code_b64.clone(),
        status: TaskStatus::Completed,
        result: Some(serde_json::json!("success")),
        capabilities: task.capabilities,
    };
    let state = Arc::new(BrokerState::new(storage));
    let state_cl = Arc::clone(&state);
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Fetch,
        WorkerAction::ReportResult(result.clone()),
    ]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events.contains(&WorkerEvent::SentFetch));
    assert!(events.contains(&WorkerEvent::ReceivedTask));
    assert!(events.contains(&WorkerEvent::SentReportResult));
    assert_eq!(
        *state_cl
            .storage
            .results
            .read()
            .await
            .get(&result.id)
            .unwrap(),
        result
    );
}

#[tokio::test]
async fn test_report_result_failure() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let task = create_test_task("test_task", TaskPriority::High).await;
    storage.enqueue_task(task.clone()).await.unwrap();
    let result = TaskResult {
        id: task.id,
        owner_id: task.owner_id,
        name: task.name.clone(),
        code_b64: task.code_b64.clone(),
        status: TaskStatus::Failed,
        result: Some(serde_json::json!("error")),
        capabilities: task.capabilities,
    };
    let state = Arc::new(BrokerState::new(storage));
    let state_cl = Arc::clone(&state);
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Fetch,
        WorkerAction::ReportResult(result.clone()),
    ]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events.contains(&WorkerEvent::SentFetch));
    assert!(events.contains(&WorkerEvent::ReceivedTask));
    assert!(events.contains(&WorkerEvent::SentReportResult));
    assert_eq!(
        *state_cl
            .storage
            .results
            .read()
            .await
            .get(&result.id)
            .unwrap(),
        result
    );
}

#[tokio::test]
async fn test_error_handling() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![WorkerAction::Fetch]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentFetch));
    assert!(events.contains(&WorkerEvent::FetchError));
}

#[tokio::test]
async fn test_multiple_workers() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let task1 = create_test_task("task1", TaskPriority::High).await;
    let task2 = create_test_task("task2", TaskPriority::High).await;
    storage.enqueue_task(task1.clone()).await.unwrap();
    storage.enqueue_task(task2.clone()).await.unwrap();
    let state = Arc::new(BrokerState::new(storage));
    let state_cl = Arc::clone(&state);
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let result1 = TaskResult {
        id: task1.id,
        owner_id: task1.owner_id,
        name: task1.name.clone(),
        code_b64: task1.code_b64.clone(),
        status: TaskStatus::Completed,
        result: Some(serde_json::json!("done1")),
        capabilities: task1.capabilities,
    };
    let result2 = TaskResult {
        id: task2.id,
        owner_id: task2.owner_id,
        name: task2.name.clone(),
        code_b64: task2.code_b64.clone(),
        status: TaskStatus::Completed,
        result: Some(serde_json::json!("done2")),
        capabilities: task2.capabilities,
    };

    let mut tw1 = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Fetch,
        WorkerAction::ReportResult(result1.clone()),
    ]));
    tw1.worker_id = "worker1".to_string();
    let (events1, handle1) = tw1.run(port).await;

    let mut tw2 = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Fetch,
        WorkerAction::ReportResult(result2.clone()),
    ]));
    tw2.worker_id = "worker2".to_string();
    let (events2, handle2) = tw2.run(port).await;

    handle1.await.unwrap();
    handle2.await.unwrap();

    let events1 = events1.lock().unwrap();
    assert!(events1.contains(&WorkerEvent::SentRegister));
    assert!(events1.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events1.contains(&WorkerEvent::SentFetch));
    assert!(events1.contains(&WorkerEvent::ReceivedTask));
    assert!(events1.contains(&WorkerEvent::SentReportResult));

    let events2 = events2.lock().unwrap();
    assert!(events2.contains(&WorkerEvent::SentRegister));
    assert!(events2.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events2.contains(&WorkerEvent::SentFetch));
    assert!(events2.contains(&WorkerEvent::ReceivedTask));
    assert!(events2.contains(&WorkerEvent::SentReportResult));
    assert_eq!(state_cl.storage.results.read().await.len(), 2);
    assert_eq!(
        *state_cl
            .storage
            .results
            .read()
            .await
            .get(&task1.id)
            .unwrap(),
        result1
    );
    assert_eq!(
        *state_cl
            .storage
            .results
            .read()
            .await
            .get(&task2.id)
            .unwrap(),
        result2
    );
}

#[tokio::test]
async fn test_shutdown_worker() {
    init_tracing();
    let storage = InMemoryStorage::new();
    let state = Arc::new(BrokerState::new(storage));
    let state_cl = Arc::clone(&state);
    let port = get_random_available_port().await;
    tokio::spawn(create_jrpc_server(state, port.into()));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let tw = TestWorker::new(TestWorkerWorkflow::Custom(vec![
        WorkerAction::Register,
        WorkerAction::Heartbeat,
        WorkerAction::Shutdown,
    ]));
    let (events, handle) = tw.run(port).await;

    handle.await.unwrap();
    let events = events.lock().unwrap();
    assert!(events.contains(&WorkerEvent::SentRegister));
    assert!(events.contains(&WorkerEvent::ReceivedRegisterResponse));
    assert!(events.contains(&WorkerEvent::SentShutdown));
    assert_eq!(state_cl.storage.worker_sessions.read().await.len(), 0);
}
