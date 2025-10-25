use aether_common::jrpc::{JsonRpcRequest, JsonRpcResponse};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::mpsc,
    time::Duration,
};
use tracing::error;

async fn register_worker(
    reader: &mut BufReader<OwnedReadHalf>,
    writer: &mut OwnedWriteHalf,
    worker_id: &str,
) -> anyhow::Result<()> {
    let register_worker_body = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: "1".into(),
        method: "register_worker".into(),
        params: json!({
            "worker_id": worker_id.to_string(),
        }),
    };
    let body = serde_json::to_string(&register_worker_body)?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n{}", body.len(), body).as_bytes())
        .await?;

    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.starts_with("Content-Length: ") {
        let len = line
            .trim_start_matches("Content-Length: ")
            .trim()
            .parse::<usize>()?;

        reader.read_line(&mut line).await?; // Read empty line.
        let mut body = vec![0; len];
        reader.read_exact(&mut body).await?;

        let response: JsonRpcResponse = serde_json::from_str(&line)?;
        if let Some(res) = response.result
            && res == json!({"status": "registered"})
        {
            Ok(())
        } else {
            anyhow::bail!("Register_worker response was not correct.");
        }
    } else {
        anyhow::bail!("Could not read bytes of register_worker response");
    }
}

pub async fn run_app(remote_rpc_server_ip: &str, worker_id: &str) -> anyhow::Result<()> {
    let stream = TcpStream::connect(remote_rpc_server_ip).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let (tx, mut rx) = mpsc::channel::<String>(10);

    register_worker(&mut reader, &mut writer, worker_id).await?;

    // Heartbeat task
    let heartbeat_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let heartbeat_message = json!({
                "jsonrpc": "2.0",
                "method": "heartbeat",
                "params": {
                    "worker_id": "worker1",
                }
            });
            let msg = format!(
                "Content-Length: {}\r\n\r\n{}",
                heartbeat_message.to_string().len(),
                heartbeat_message
            );
            if heartbeat_tx.send(msg).await.is_err() {
                break; // Writer dropped, just break and let the program crash.
            }
        }
    });

    // Writer task
    let writer_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = writer.write_all(msg.as_bytes()).await {
                error!("[ERROR] Failed to write message to socket: {e}");
                break; // Break to kill task and crash the program.
            }
        }
    });

    // Reader task
    let reader_task = tokio::spawn(async move {
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

                // TODO: Process message.

                // match process_jsonrpc_message(&message_body, &state).await {
                //     Ok(Some(response)) => {
                //         // Message was a request.
                //         let res = serde_json::to_string(&response);
                //         match res {
                //             Ok(res_str) => {
                //                 let response_bytes =
                //                     format!("Content-Length: {}\r\n\r\n{}", res_str.len(), res_str);
                //                 writer.write_all(response_bytes.as_bytes()).await.unwrap();
                //             }
                //             Err(_) => continue,
                //         }
                //     }
                //     Ok(None) => continue, // Was just a notification.
                //     Err(_) => continue,
                // }
            } else {
                // Incorrect message, just continue
                continue;
            }
        }
    });

    tokio::select! {
        _ = writer_task => error!("[ERROR] Writer task crashed."),
        _ = reader_task => error!("[ERROR] Reader task crashed."),
    }
    Ok(())
}
