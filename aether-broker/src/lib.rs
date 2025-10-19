mod api;

use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

pub use api::build_router;
pub use api::AppState;

pub async fn run_app() -> anyhow::Result<()> {
    let state = AppState {
        queue: Arc::new(RwLock::new(HashMap::new())),
        results: Arc::new(RwLock::new(HashMap::new())),
    };
    let app = api::build_router(state);
    let listener = TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Could not bind Broker API to 0.0.0.0:8080");

    axum::serve(listener, app).await.expect("Could not serve Broker API");

    Ok(())
}
