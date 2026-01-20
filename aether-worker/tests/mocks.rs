mod test_utils;

#[cfg(test)]
mod tests {
    use crate::test_utils::BrokerEvent;

    use super::test_utils::{TestBroker, TestBrokerWorkflow, TestRegisterBrokerResponse};
    use aether_core::capabilities::{CPUArchitecture, WorkerCapabilities};
    use aether_worker::Worker;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_register_worker_is_ok() {
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
        assert_eq!(
            *lock,
            vec![
                BrokerEvent::RegistrationAttempt,
                BrokerEvent::RegistrationOk
            ]
        );
    }

    #[tokio::test]
    async fn test_register_worker_retries() {
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
}
