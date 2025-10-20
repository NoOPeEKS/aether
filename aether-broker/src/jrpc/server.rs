use crate::jrpc::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::state::BrokerState;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

pub async fn create_jrpc_server(state: BrokerState, port: usize) {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap_or_else(|_| panic!("Could not bind JRPC server to 0.0.0.0:{port}"));

    loop {
        if let Ok((stream, addr)) = listener.accept().await {
            info!("[INFO] Accepted TCP connection from {}", addr);
            tokio::spawn(handle_jrpc_connection(stream));
        } else {
            info!("[INFO] Could not accept an incoming connection from");
        }
    }
}

async fn handle_jrpc_connection(stream: TcpStream) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();

        let read = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(_) => continue,
        };

        if read == 0 {
            break;
        }

        if line.starts_with("Content-Length: ") {
            // Correct message
            let len = match line
                .trim_start_matches("Content-Length: ")
                .trim()
                .parse::<usize>()
            {
                Ok(len) => len,
                Err(_) => {
                    // Invalid content length, just continue.
                    continue;
                }
            };

            let mut empty_line = String::new();
            if reader.read_line(&mut empty_line).await.is_err() {
                continue;
            }

            let mut message_body = vec![0; len];
            if reader.read_exact(&mut message_body).await.is_err() {
                continue;
            }

            match process_jsonrpc_message(&message_body).await {
                Ok(_) => {}
                Err(_) => continue,
            }
        } else {
            // Incorrect message, just continue
            continue;
        }
    }
}

async fn process_jsonrpc_message(message: &[u8]) -> anyhow::Result<()> {
    let message = String::from_utf8_lossy(message);
    let message: serde_json::Value = serde_json::from_str(&message)?;
    if message.get("id").is_some() {
        // It's a request
        let request: JsonRpcRequest = serde_json::from_value(message)?;
        println!("{:?}", request);
    } else {
        // It's a notification
        let notification: JsonRpcNotification = serde_json::from_value(message)?;
        println!("{:?}", notification);
    }
    Ok(())
}
