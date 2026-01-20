use aether_core::jrpc::{JsonRpcError, JsonRpcErrorCode};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

mod jrpc;

use jrpc::{ManualRpcServer, handle_connection};

#[derive(Clone)]
pub enum TestBrokerWorkflow {
    RegisterWorker(TestRegisterBrokerResponse),
}

#[derive(Clone)]
pub enum TestRegisterBrokerResponse {
    Success,
    Error,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BrokerEvent {
    RegistrationAttempt,
    RegistrationOk,
    RegistrationError,
    Reconnection,
}

pub struct TestBroker {
    workflow: TestBrokerWorkflow,
    events: Arc<Mutex<Vec<BrokerEvent>>>,
}

impl TestBroker {
    pub fn new(workflow: TestBrokerWorkflow) -> Self {
        Self {
            workflow,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn run(self) -> (u16, Arc<Mutex<Vec<BrokerEvent>>>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind");
        let port = listener.local_addr().unwrap().port();

        let workflow = Arc::new(Mutex::new(self.workflow));
        let mut server = ManualRpcServer::new();
        let workflow_clone = Arc::clone(&workflow);

        let events = Arc::clone(&self.events);
        server.add_method("register_worker".to_string(), move |_params: Value| {
            events
                .lock()
                .unwrap()
                .push(BrokerEvent::RegistrationAttempt);
            let wf = workflow_clone.lock().unwrap();
            match *wf {
                TestBrokerWorkflow::RegisterWorker(ref resp) => match resp {
                    TestRegisterBrokerResponse::Success => {
                        events.lock().unwrap().push(BrokerEvent::RegistrationOk);
                        Ok(serde_json::json!({"status": "registered"}))
                    }
                    TestRegisterBrokerResponse::Error => {
                        events.lock().unwrap().push(BrokerEvent::RegistrationError);
                        Err(JsonRpcError {
                            code: JsonRpcErrorCode::InvalidRequest,
                            message: "Invalid Request".into(),
                            data: None,
                        })
                    }
                },
            }
        });

        let handlers = Arc::new(server.handlers);
        let handle = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let handlers = Arc::clone(&handlers);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, handlers).await {
                        eprintln!("Connection error: {:?}", e);
                    }
                });
            }
        });

        (port, self.events, handle)
    }
}
