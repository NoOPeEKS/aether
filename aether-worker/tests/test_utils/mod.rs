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
    FetchTask(TestFetchTaskResponse),
    FetchTaskAndCancel(TestFetchTaskResponse),
    Heartbeat,
}

#[derive(Clone)]
pub enum TestRegisterBrokerResponse {
    Success,
    Error,
}

#[derive(Clone)]
pub enum TestFetchTaskResponse {
    Task(aether_core::task::Task),
    NoTask,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BrokerEvent {
    RegistrationAttempt,
    RegistrationOk,
    RegistrationError,
    FetchTaskAttempt,
    TaskAssigned,
    NoTaskAvailable,
    HeartbeatReceived,
    ResultReported,
    WorkerShutdown,
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
                TestBrokerWorkflow::RegisterWorker(TestRegisterBrokerResponse::Error) => {
                    events.lock().unwrap().push(BrokerEvent::RegistrationError);
                    Err(JsonRpcError {
                        code: JsonRpcErrorCode::InvalidRequest,
                        message: "Invalid Request".into(),
                        data: None,
                    })
                }
                _ => {
                    events.lock().unwrap().push(BrokerEvent::RegistrationOk);
                    Ok(serde_json::json!({"status": "registered"}))
                }
            }
        });

        let events = Arc::clone(&self.events);
        let workflow_clone = Arc::clone(&workflow);
        server.add_method("fetch_task".to_string(), move |_params: Value| {
            events.lock().unwrap().push(BrokerEvent::FetchTaskAttempt);
            let wf = workflow_clone.lock().unwrap();
            match *wf {
                TestBrokerWorkflow::FetchTask(ref resp)
                | TestBrokerWorkflow::FetchTaskAndCancel(ref resp) => match resp {
                    TestFetchTaskResponse::Task(task) => {
                        events.lock().unwrap().push(BrokerEvent::TaskAssigned);
                        Ok(serde_json::json!({"task": task}))
                    }
                    TestFetchTaskResponse::NoTask => {
                        events.lock().unwrap().push(BrokerEvent::NoTaskAvailable);
                        Ok(serde_json::json!({}))
                    }
                },
                _ => Err(JsonRpcError {
                    code: JsonRpcErrorCode::MethodNotFound,
                    message: "Method not found".into(),
                    data: None,
                }),
            }
        });

        let events = Arc::clone(&self.events);
        server.add_method("heartbeat".to_string(), move |_params: Value| {
            events.lock().unwrap().push(BrokerEvent::HeartbeatReceived);
            // Notifications don't expect response, but handler returns value
            // gotta do this hack bc of the way we coded this.
            Ok(serde_json::json!({})) 
        });

        let events = Arc::clone(&self.events);
        server.add_method("report_result".to_string(), move |_params: Value| {
            // Notifications don't expect response, but handler returns value
            // gotta do this hack bc of the way we coded this.
            events.lock().unwrap().push(BrokerEvent::ResultReported);
            Ok(serde_json::json!({}))
        });

        let events = Arc::clone(&self.events);
        server.add_method("worker_shutdown".to_string(), move |_params: Value| {
            // Notifications don't expect response, but handler returns value
            // gotta do this hack bc of the way we coded this.
            events.lock().unwrap().push(BrokerEvent::WorkerShutdown);
            Ok(serde_json::json!({}))
        });

        let handlers = Arc::new(server.handlers);
        let handle = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let handlers = Arc::clone(&handlers);
                let workflow = Arc::clone(&workflow);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, handlers, workflow).await {
                        eprintln!("Connection error: {:?}", e);
                    }
                });
            }
        });

        (port, self.events, handle)
    }
}
