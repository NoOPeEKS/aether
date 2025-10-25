use std::sync::Once;
static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt().init();
    });
}

#[tokio::test]
async fn test_register_worker() {
    init_tracing();
    tokio::spawn(aether_broker::run_app(8080, 8081));
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    tokio::spawn(aether_worker::run_app("127.0.0.1:8081", "test-worker", 10));
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
}
