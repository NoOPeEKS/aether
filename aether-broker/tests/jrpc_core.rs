use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

use aether_broker::jrpc::server::create_jrpc_server;
use aether_broker::state::BrokerState;
use aether_common::jrpc::{JsonRpcNotification, JsonRpcRequest};
use serde_json::json;
use tokio::net::TcpStream;
use tracing_subscriber::fmt::format;

use std::sync::Once;
static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

#[tokio::test]
async fn test_parsing() {
    init_tracing();
    let state = BrokerState::new(10);
    tokio::spawn(create_jrpc_server(state, 6969));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let mut stream = TcpStream::connect("127.0.0.1:6969").await.unwrap();
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: "worker1-id1".to_string(),
        method: "dummy_method".to_string(),
        params: json!({"arg1": 2, "arg2": "hola"}),
    };
    let body = serde_json::to_string(&request).unwrap();
    stream
        .write_all(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
        .await
        .unwrap();

    stream.flush().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn test_register_worker() {
    init_tracing();
    let state = BrokerState::new(10);
    tokio::spawn(create_jrpc_server(state, 7777));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let mut stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: "worker1-id1".to_string(),
        method: "register_worker".to_string(),
        params: json!({"worker_id": "test-worker-1"}),
    };
    let body = serde_json::to_string(&request).unwrap();
    stream
        .write_all(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
        .await
        .unwrap();

    stream.flush().await.unwrap();

    tokio::time::sleep(Duration::from_secs(4)).await;

    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    _ = reader.read_line(&mut buf).await;
    _ = reader.read_line(&mut buf).await;
    let mut buf: [u8; 82] = [0; 82];
    _ = reader.read_exact(&mut buf).await;

    println!("{}", String::from_utf8_lossy(&buf));
}

#[tokio::test]
async fn test_heartbeat() {
    init_tracing();
    let state = BrokerState::new(10);
    tokio::spawn(create_jrpc_server(state, 6666));

    tokio::time::sleep(Duration::from_secs(5)).await;

    let stream = TcpStream::connect("127.0.0.1:6666").await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: "worker1-id1".to_string(),
        method: "register_worker".to_string(),
        params: json!({"worker_id": "test-worker-1"}),
    };
    let body = serde_json::to_string(&request).unwrap();
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
        .await
        .unwrap();

    writer.flush().await.unwrap();

    tokio::time::sleep(Duration::from_secs(4)).await;

    let mut reader = BufReader::new(reader);
    let mut buf = String::new();
    _ = reader.read_line(&mut buf).await;
    _ = reader.read_line(&mut buf).await;
    let mut buf: [u8; 82] = [0; 82];
    _ = reader.read_exact(&mut buf).await;

    println!("{}", String::from_utf8_lossy(&buf));

    let heartbeat = JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: "heartbeat".into(),
        params: json!({"worker_id": "test-worker-1"}),
    };
    let hb_body = serde_json::to_string(&heartbeat).unwrap();
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n{}", hb_body.len(), hb_body).as_bytes())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
}
