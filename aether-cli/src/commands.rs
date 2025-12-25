use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Aether distributed task queue CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Subcommands for spawning and handling an Aether broker.")]
    Broker {
        #[command(subcommand)]
        command: BrokerCommands,
    },
    #[command(about = "Subcommands for spawning and handling an Aether worker.")]
    Worker {
        #[command(subcommand)]
        command: WorkerCommands,
    },
    #[command(
        about = "Launch an interactive terminal user interface for handling an Aether cluster."
    )]
    Tui {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_port: usize,
    },
}

#[derive(Subcommand)]
pub enum BrokerCommands {
    #[command(about = "Starts an Aether broker with the specified ports.")]
    Start {
        #[arg(long)]
        api_port: usize,

        #[arg(long)]
        jrpc_port: usize,
    },
}

#[derive(Subcommand)]
pub enum WorkerCommands {
    #[command(about = "Starts an Aether worker that connects to an existing broker.")]
    Start {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_port: usize,
    },
}
