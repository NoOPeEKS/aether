use aether_core::jrpc::{
    JsonRpcError, JsonRpcErrorCode, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    format_jrpc_message,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

type JsonRpcResult = Result<Value, JsonRpcError>;
type MethodHandler = Box<dyn Fn(Value) -> JsonRpcResult + Send + Sync>;

pub struct ManualRpcServer {
    pub handlers: HashMap<String, MethodHandler>,
}

impl ManualRpcServer {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn add_method<F>(&mut self, name: String, handler: F)
    where
        F: Fn(Value) -> JsonRpcResult + Send + Sync + 'static,
    {
        self.handlers.insert(name, Box::new(handler));
    }
}

pub async fn handle_connection(
    stream: TcpStream,
    handlers: Arc<HashMap<String, MethodHandler>>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    loop {
        match read_framed_message(&mut reader).await {
            Ok(message) => match message {
                JsonRpcMessage::Request(request) => {
                    let response = match handlers.get(&request.method) {
                        Some(handler) => match handler(request.params) {
                            Ok(result) => JsonRpcResponse {
                                jsonrpc: "2.0".into(),
                                id: request.id,
                                result: Some(result),
                                error: None,
                            },
                            Err(err) => JsonRpcResponse {
                                jsonrpc: "2.0".into(),
                                id: request.id,
                                result: None,
                                error: Some(err),
                            },
                        },
                        None => JsonRpcResponse {
                            jsonrpc: "2.0".into(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: JsonRpcErrorCode::MethodNotFound,
                                message: "Method not found".into(),
                                data: None,
                            }),
                        },
                    };

                    let msg = format_jrpc_message(response)?;
                    writer.write_all(msg.as_bytes()).await?;
                }
                JsonRpcMessage::Notification(notification) => {
                    if let Some(handler) = handlers.get(&notification.method) {
                        let _ = handler(notification.params);
                    }
                    // No response for notifications
                }
            },
            Err(e) if e.to_string().contains("EOF") => break,
            Err(e) => {
                eprintln!("Connection error: {}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn read_framed_message(
    reader: &mut BufReader<OwnedReadHalf>,
) -> anyhow::Result<JsonRpcMessage> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        anyhow::bail!("EOF");
    }
    if !line.starts_with("Content-Length: ") {
        anyhow::bail!("Invalid framing: expected Content-Length");
    }
    let len_str = line.trim_start_matches("Content-Length: ").trim();
    if len_str.is_empty() {
        anyhow::bail!("Invalid framing: empty content length");
    }
    let len: usize = len_str.parse()?;
    line.clear();
    reader.read_line(&mut line).await?; // empty line
    if !line.trim().is_empty() {
        anyhow::bail!("Invalid framing: expected empty line");
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).await?;
    let value: serde_json::Value = serde_json::from_slice(&body)?;
    if value.get("id").is_some() {
        let request: JsonRpcRequest = serde_json::from_value(value)?;
        Ok(JsonRpcMessage::Request(request))
    } else {
        let notification: JsonRpcNotification = serde_json::from_value(value)?;
        Ok(JsonRpcMessage::Notification(notification))
    }
}
