use aether_core::jrpc::{
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, format_jrpc_message,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};

pub async fn get_random_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

pub async fn send_jrpc_request(
    writer: &mut WriteHalf<TcpStream>,
    request: JsonRpcRequest,
) -> anyhow::Result<()> {
    let message = format_jrpc_message(request)?;
    writer.write_all(message.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_jrpc_response(
    reader: &mut BufReader<ReadHalf<TcpStream>>,
) -> anyhow::Result<JsonRpcResponse> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.starts_with("Content-Length: ") {
        let len = line
            .trim_start_matches("Content-Length: ")
            .trim()
            .parse::<usize>()?;
        reader.read_line(&mut line).await?; // empty line
        let mut body = vec![0; len];
        reader.read_exact(&mut body).await?;
        let response: JsonRpcResponse = serde_json::from_slice(&body)?;
        Ok(response)
    } else {
        anyhow::bail!("Invalid response framing");
    }
}

pub async fn send_jrpc_notification(
    writer: &mut WriteHalf<TcpStream>,
    notification: JsonRpcNotification,
) -> anyhow::Result<()> {
    let message = format_jrpc_message(notification)?;
    writer.write_all(message.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
