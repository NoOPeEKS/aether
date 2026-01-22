mod test_utils;

#[cfg(test)]
mod tests {
    use crate::test_utils::BrokerEvent;

    use super::test_utils::{
        TestBroker, TestBrokerWorkflow, TestFetchTaskResponse, TestRegisterBrokerResponse,
    };
    use std::sync::Once;
    use aether_core::capabilities::{CPUArchitecture, TaskCapabilities, WorkerCapabilities};
    use aether_core::task::{Task, TaskPriority};
    use aether_worker::Worker;
    use base64::prelude::*;
    use tokio::time::Duration;
    use uuid::Uuid;

    static INIT: Once = Once::new();

    fn init_tracing() {
        INIT.call_once(|| {
            tracing_subscriber::fmt().init();
        });
    }

    #[tokio::test]
    async fn test_worker_task_execution_success() {
        init_tracing();
        let sample_task = Task {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            name: "test-task".to_string(),
            code_b64: BASE64_STANDARD.encode("print('hello world')"),
            priority: TaskPriority::Medium,
            capabilities: Some(TaskCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            }),
        };
        let test_broker = TestBroker::new(TestBrokerWorkflow::FetchTask(
            TestFetchTaskResponse::Task(sample_task.clone()),
        ));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            1, // max concurrent 1
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
        assert!(lock.contains(&BrokerEvent::TaskAssigned));
        assert!(lock.contains(&BrokerEvent::ResultReported));
    }

    #[tokio::test]
    async fn test_worker_task_execution_failure() {
        init_tracing();
        let sample_task = Task {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            name: "test-task".to_string(),
            code_b64: BASE64_STANDARD.encode("import sys; sys.exit(1)"),
            priority: TaskPriority::Medium,
            capabilities: Some(TaskCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            }),
        };
        let test_broker = TestBroker::new(TestBrokerWorkflow::FetchTask(
            TestFetchTaskResponse::Task(sample_task.clone()),
        ));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            1,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
        assert!(lock.contains(&BrokerEvent::TaskAssigned));
        assert!(lock.contains(&BrokerEvent::ResultReported));
    }

    #[tokio::test]
    async fn test_worker_task_cancellation() {
        init_tracing();
        let sample_task = Task {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            name: "test-task".to_string(),
            code_b64: BASE64_STANDARD.encode("import time; time.sleep(10)"), // Long running
            priority: TaskPriority::Medium,
            capabilities: Some(TaskCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            }),
        };
        let test_broker = TestBroker::new(TestBrokerWorkflow::FetchTaskAndCancel(
            TestFetchTaskResponse::Task(sample_task.clone()),
        ));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            1,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
        assert!(lock.contains(&BrokerEvent::TaskAssigned));
        assert!(lock.contains(&BrokerEvent::ResultReported));
    }

    #[tokio::test]
    async fn test_register_worker_is_ok() {
        init_tracing();
        let test_broker = TestBroker::new(TestBrokerWorkflow::RegisterWorker(
            TestRegisterBrokerResponse::Success,
        ));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            25,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::Aarch64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationAttempt));
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
    }

    #[tokio::test]
    async fn test_register_worker_retries() {
        init_tracing();
        let test_broker = TestBroker::new(TestBrokerWorkflow::RegisterWorker(
            TestRegisterBrokerResponse::Error,
        ));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            25,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::Aarch64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert_eq!(
            *lock,
            vec![
                BrokerEvent::RegistrationAttempt,
                BrokerEvent::RegistrationError,
                BrokerEvent::RegistrationAttempt,
                BrokerEvent::RegistrationError,
            ]
        );
    }

    #[tokio::test]
    async fn test_worker_task_fetch_success() {
        init_tracing();
        let sample_task = Task {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            name: "test-task".to_string(),
            code_b64: BASE64_STANDARD.encode("print('hello')"),
            priority: TaskPriority::Medium,
            capabilities: Some(TaskCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            }),
        };
        let test_broker = TestBroker::new(TestBrokerWorkflow::FetchTask(
            TestFetchTaskResponse::Task(sample_task.clone()),
        ));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            25,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
        assert!(lock.contains(&BrokerEvent::FetchTaskAttempt));
        assert!(lock.contains(&BrokerEvent::TaskAssigned));
    }

    #[tokio::test]
    async fn test_worker_task_fetch_no_compatible_tasks() {
        init_tracing();
        let test_broker =
            TestBroker::new(TestBrokerWorkflow::FetchTask(TestFetchTaskResponse::NoTask));
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            25,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
        assert!(lock.contains(&BrokerEvent::FetchTaskAttempt));
        assert!(lock.contains(&BrokerEvent::NoTaskAvailable));
    }

    #[tokio::test]
    async fn test_worker_heartbeat_success() {
        init_tracing();
        let test_broker = TestBroker::new(TestBrokerWorkflow::Heartbeat);
        let (port, events, handle) = test_broker.run().await;
        let addr = format!("127.0.0.1:{port}");
        let worker = Worker::new(
            "test-worker",
            &addr,
            25,
            WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(12)) => { // Wait for heartbeat
                handle.abort();
            }
            _ = worker.run() => {
                handle.abort();
            }
        };
        println!("EVENTS: {:?}", events.lock().unwrap());
        let lock = events.lock().unwrap();
        assert!(lock.contains(&BrokerEvent::RegistrationOk));
        assert!(lock.contains(&BrokerEvent::HeartbeatReceived));
    }
}
