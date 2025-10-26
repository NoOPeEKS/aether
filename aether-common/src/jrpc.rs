use std::fmt::Display;

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
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonRpcError {
    pub code: JsonRpcErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl Display for JsonRpcErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonRpcErrorCode::ParseError => write!(f, "Parse Error (-32700)"),
            JsonRpcErrorCode::InvalidRequest => write!(f, "InvalidRequest Error (-32600)"),
            JsonRpcErrorCode::MethodNotFound => write!(f, "MethodNotFound Error (-32601)"),
            JsonRpcErrorCode::InvalidParams => write!(f, "InvalidParams Error (-32602)"),
            JsonRpcErrorCode::InternalError => write!(f, "Internal Error (-32603)"),
            JsonRpcErrorCode::ServerError => write!(f, "Server Error"),
        }
    }
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
