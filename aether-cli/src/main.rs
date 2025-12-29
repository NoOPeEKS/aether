mod commands;

use crate::commands::{BrokerCommands, Cli, Commands, WorkerCommands};
use aether_broker::DefaultBroker;
use aether_core::{
    broker::storage::InMemoryStorage, capabilities::WorkerCapabilities, traits::Broker,
};
use aether_worker::Worker;
use tracing::info;
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
                worker_id,
                broker_ip,
                broker_port,
                gpu,
                arch,
            } => {
                let worker_capabilities = WorkerCapabilities {
                    gpu,
                    arch: arch.to_worker_arch(),
                };
                let ip = format!("{broker_ip}:{broker_port}");
                let worker = Worker::new(&worker_id, &ip, 10, worker_capabilities);
                let shutdown_token = worker.shutdown_token.clone();
                tokio::select! {
                    _ = worker.run() => {

                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("[INFO] Gracefully shutting down worker and executing tasks...");
                        shutdown_token.cancel();
                    }
                }
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
