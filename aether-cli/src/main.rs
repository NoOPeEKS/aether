mod commands;

use crate::commands::{BrokerCommands, Cli, Commands, WorkerCommands};
use aether_broker::DefaultBroker;
use aether_core::{
    broker::storage::InMemoryStorage,
    capabilities::{WorkerCPUArchitecture, WorkerCapabilities},
    traits::Broker,
};
use aether_worker::Worker;
use clap::Parser;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Broker { command } => match command {
            BrokerCommands::Start {
                api_port,
                jrpc_port,
            } => {
                let storage = InMemoryStorage::new();
                let broker = DefaultBroker::new(storage);
                broker
                    .run(api_port, jrpc_port)
                    .await
                    .expect("Broker should run");
            }
        },
        Commands::Worker { command } => match command {
            WorkerCommands::Start {
                broker_ip,
                broker_port,
            } => {
                let worker_capabilities = WorkerCapabilities {
                    gpu: false,
                    arch: WorkerCPUArchitecture::X86_64,
                };
                let ip = format!("{broker_ip}:{broker_port}");
                let worker = Worker::new("test-worker-id", &ip, 10, worker_capabilities);
                worker.run().await.expect("Worker should run");
            }
        },
        Commands::Tui {
            broker_ip: _,
            broker_port: _,
        } => {
            println!("In the future, TUI will execute here.");
        }
    }
}
