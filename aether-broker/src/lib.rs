pub mod api;
pub mod jrpc;
pub mod state;

use std::collections::HashMap;
use std::sync::Arc;

use aether_core::traits::{Broker, Storage};
pub use api::build_router;
use jrpc::server::create_jrpc_server;
pub use state::BrokerState;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

pub struct DefaultBroker<S>
where
    S: Storage,
{
    pub state: Arc<BrokerState<S>>,
}

#[async_trait::async_trait]
impl<S> Broker for DefaultBroker<S>
where
    S: Storage,
{
    async fn run(&self, http_port: usize, rpc_port: usize) -> anyhow::Result<()> {
        tokio::try_join!(
            self.run_http_server(http_port),
            self.run_jrpc_server(rpc_port)
        )?;
        Ok(())
    }
    async fn run_http_server(&self, port: usize) -> anyhow::Result<()> {
        let axum_state = Arc::clone(&self.state);
        let app = api::build_router(axum_state);
        let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .unwrap_or_else(|_| panic!("Could not bind Broker API to 0.0.0.0:{port}"));
        axum::serve(listener, app).await?;
        Ok(())
    }
    async fn run_jrpc_server(&self, port: usize) -> anyhow::Result<()> {
        let jrpc_state = Arc::clone(&self.state);
        create_jrpc_server(jrpc_state, port).await;
        Ok(())
    }
}

impl<S> DefaultBroker<S>
where
    S: Storage,
{
    pub fn new(storage: S) -> Self {
        Self {
            state: Arc::new(BrokerState {
                storage,
                worker_sessions: RwLock::new(HashMap::new()),
            }),
        }
    }
}
