pub mod storage;

#[async_trait::async_trait]
pub trait Broker {
    async fn run(&self) -> anyhow::Result<()>;
    async fn run_http_server(&self, port: usize) -> anyhow::Result<()>;
    async fn run_jrpc_server(&self, port: usize) -> anyhow::Result<()>;
}
