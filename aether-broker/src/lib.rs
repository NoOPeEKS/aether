pub mod api;
mod state;

use tokio::net::TcpListener;

pub use api::build_router;
pub use state::BrokerState;

pub async fn run_app(port: usize) -> anyhow::Result<()> {
    let state = BrokerState::new(10);
    let app = api::build_router(state);
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap_or_else(|_| panic!("Could not bind Broker API to 0.0.0.0:{port}"));

    axum::serve(listener, app)
        .await
        .expect("Could not serve Broker API");

    Ok(())
}
