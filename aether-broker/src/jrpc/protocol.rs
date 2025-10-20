use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: String, // "workerX-id_here"
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: String, // "workerX-id_here"
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonRpcError {
    pub code: JsonRpcErrorCode,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum JsonRpcErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    ServerError,
}

impl From<isize> for JsonRpcErrorCode {
    fn from(value: isize) -> Self {
        match value {
            -32700 => Self::ParseError,
            -32600 => Self::InvalidRequest,
            -32601 => Self::MethodNotFound,
            -32602 => Self::InvalidParams,
            -32603 => Self::InternalError,
            -32099..-32000 => Self::ServerError,
            _ => panic!("Non implemented error code"),
        }
    }
}

impl From<JsonRpcErrorCode> for isize {
    fn from(value: JsonRpcErrorCode) -> Self {
        match value {
            JsonRpcErrorCode::ParseError => -32700,
            JsonRpcErrorCode::InvalidRequest => -32600,
            JsonRpcErrorCode::MethodNotFound => -32601,
            JsonRpcErrorCode::InvalidParams => -32602,
            JsonRpcErrorCode::InternalError => -32603,
            // TODO: For now we only have custom server -32001
            JsonRpcErrorCode::ServerError => -32001,
        }
    }
}
