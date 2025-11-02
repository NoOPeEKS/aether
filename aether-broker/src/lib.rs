pub mod api;
pub mod jrpc;
pub mod state;

use std::sync::Arc;
use jrpc::server::create_jrpc_server;
use tokio::net::TcpListener;

pub use api::build_router;
pub use state::BrokerState;

pub async fn run_app(http_server_port: usize, rpc_server_port: usize) -> anyhow::Result<()> {
    let state = Arc::new(BrokerState::new(10));
    let jrpc_state = Arc::clone(&state);

    let app = api::build_router(state);
    let listener = TcpListener::bind(format!("0.0.0.0:{http_server_port}"))
        .await
        .unwrap_or_else(|_| panic!("Could not bind Broker API to 0.0.0.0:{http_server_port}"));

    tokio::try_join!(
        async {
            create_jrpc_server(jrpc_state, rpc_server_port).await;
            Ok::<(), anyhow::Error>(())
        },
        async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| anyhow::anyhow!("Axum server error: {}", e))
        }
    )?;

    Ok(())
}
