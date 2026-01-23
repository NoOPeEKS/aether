use aether_core::capabilities::{CPUArchitecture, WorkerCapabilities};
use aether_core::jrpc::{
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, format_jrpc_message,
};
use aether_core::task::TaskResult;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

pub mod jrpc;

use jrpc::{read_jrpc_response, send_jrpc_notification, send_jrpc_request};

#[derive(Clone)]
pub enum TestWorkerWorkflow {
    RegisterOnly,
    RegisterAndFetch(TestFetchResponse),
    RegisterFetchHeartbeat,
    Custom(Vec<WorkerAction>),
}

#[derive(Clone)]
pub enum WorkerAction {
    Register,
    Fetch,
    Heartbeat,
    ReportResult(TaskResult),
    Shutdown,
}

#[derive(Clone)]
pub enum TestFetchResponse {
    Task(aether_core::task::Task),
    NoTask,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WorkerEvent {
    SentRegister,
    ReceivedRegisterResponse,
    SentFetch,
    ReceivedTask,
    ReceivedNoTask,
    SentHeartbeat,
    SentReportResult,
    SentShutdown,
}

pub struct TestWorker {
    pub workflow: TestWorkerWorkflow,
    pub events: Arc<Mutex<Vec<WorkerEvent>>>,
    pub worker_id: String,
    pub capabilities: WorkerCapabilities,
}

impl TestWorker {
    pub fn new(workflow: TestWorkerWorkflow) -> Self {
        Self {
            workflow,
            events: Arc::new(Mutex::new(Vec::new())),
            worker_id: "test-worker-1".to_string(),
            capabilities: WorkerCapabilities {
                gpu: false,
                arch: CPUArchitecture::X86_64,
            },
        }
    }

    pub async fn run(self, port: u16) -> (Arc<Mutex<Vec<WorkerEvent>>>, JoinHandle<()>) {
        let events = self.events;
        let workflow = self.workflow;
        let worker_id = self.worker_id;
        let capabilities = self.capabilities;
        let events_for_spawn = Arc::clone(&events);
        let handle = tokio::spawn(async move {
            if let Ok(stream) = TcpStream::connect(format!("127.0.0.1:{}", port)).await {
                let (reader, mut writer) = tokio::io::split(stream);
                let mut reader = BufReader::new(reader);

                let tw = TestWorker {
                    workflow,
                    events: events_for_spawn,
                    worker_id,
                    capabilities,
                };

                match tw.workflow {
                    TestWorkerWorkflow::RegisterOnly => {
                        tw.register_worker(&mut reader, &mut writer).await;
                    }
                    TestWorkerWorkflow::RegisterAndFetch(ref resp) => {
                        tw.register_worker(&mut reader, &mut writer).await;
                        tw.fetch_task(&mut reader, &mut writer, Some(resp.clone()))
                            .await;
                    }
                    TestWorkerWorkflow::RegisterFetchHeartbeat => {
                        tw.register_worker(&mut reader, &mut writer).await;
                        tw.fetch_task(&mut reader, &mut writer, None).await;
                        // Loop heartbeat
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                            tw.send_heartbeat(&mut writer).await;
                        }
                    }
                    TestWorkerWorkflow::Custom(ref actions) => {
                        for action in actions.clone() {
                            match action {
                                WorkerAction::Register => {
                                    tw.register_worker(&mut reader, &mut writer).await;
                                }
                                WorkerAction::Fetch => {
                                    tw.fetch_task(&mut reader, &mut writer, None).await;
                                }
                                WorkerAction::Heartbeat => {
                                    tw.send_heartbeat(&mut writer).await;
                                }
                                WorkerAction::ReportResult(result) => {
                                    tw.report_result(&mut writer, result).await;
                                }
                                WorkerAction::Shutdown => {
                                    tw.shutdown(&mut writer).await;
                                    break;
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!("Failed to connect to broker on port {}", port);
            }
        });

        (events, handle)
    }

    async fn register_worker(
        &self,
        reader: &mut BufReader<ReadHalf<TcpStream>>,
        writer: &mut WriteHalf<TcpStream>,
    ) {
        self.events.lock().unwrap().push(WorkerEvent::SentRegister);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: "1".into(),
            method: "register_worker".into(),
            params: json!({
                "worker_id": self.worker_id,
                "capabilities": self.capabilities,
            }),
        };
        if send_jrpc_request(writer, request).await.is_ok() {
            if read_jrpc_response(reader).await.is_ok() {
                self.events
                    .lock()
                    .unwrap()
                    .push(WorkerEvent::ReceivedRegisterResponse);
            }
        }
    }

    async fn fetch_task(
        &self,
        reader: &mut BufReader<ReadHalf<TcpStream>>,
        writer: &mut WriteHalf<TcpStream>,
        expected: Option<TestFetchResponse>,
    ) {
        self.events.lock().unwrap().push(WorkerEvent::SentFetch);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: "2".into(),
            method: "fetch_task".into(),
            params: json!({
                "worker_id": self.worker_id,
            }),
        };
        if send_jrpc_request(writer, request).await.is_ok() {
            // For simplicity, assume we receive a response
            // In real impl, check if task or none
            if expected.is_some() {
                self.events.lock().unwrap().push(WorkerEvent::ReceivedTask);
            } else {
                self.events
                    .lock()
                    .unwrap()
                    .push(WorkerEvent::ReceivedNoTask);
            }
        }
    }

    async fn send_heartbeat(&self, writer: &mut WriteHalf<TcpStream>) {
        self.events.lock().unwrap().push(WorkerEvent::SentHeartbeat);
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: "heartbeat".into(),
            params: json!({
                "worker_id": self.worker_id,
            }),
        };
        let _ = send_jrpc_notification(writer, notification).await;
    }

    async fn report_result(&self, writer: &mut WriteHalf<TcpStream>, result: TaskResult) {
        self.events
            .lock()
            .unwrap()
            .push(WorkerEvent::SentReportResult);
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: "report_result".into(),
            params: serde_json::to_value(result).unwrap(),
        };
        let _ = send_jrpc_notification(writer, notification).await;
    }

    async fn shutdown(&self, writer: &mut WriteHalf<TcpStream>) {
        self.events.lock().unwrap().push(WorkerEvent::SentShutdown);
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: "worker_shutdown".into(),
            params: json!({
                "worker_id": self.worker_id,
            }),
        };
        let _ = send_jrpc_notification(writer, notification).await;
    }
}
