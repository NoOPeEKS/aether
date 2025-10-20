use std::time::Duration;
use tokio::io::AsyncWriteExt;

use aether_broker::jrpc::protocol::JsonRpcRequest;
use aether_broker::jrpc::server::create_jrpc_server;
use aether_broker::state::BrokerState;
use serde_json::json;
use tokio::net::TcpStream;

#[tokio::test]
async fn test_parsing() {
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
