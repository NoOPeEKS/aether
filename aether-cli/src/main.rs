use clap::Parser;
mod commands;

use crate::commands::{BrokerCommands, Cli, Commands, WorkerCommands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Broker { command } => match command {
            BrokerCommands::Start {
                api_port,
                jrpc_port,
            } => {
                println!(
                    "Starting http server at 0.0.0.0:{api_port}...\nStarting jrpc server at 0.0.0.0:{jrpc_port}"
                );
            }
        },
        Commands::Worker { command } => match command {
            WorkerCommands::Start {
                broker_ip,
                broker_port,
            } => {
                println!("Connecting to broker at {broker_ip}:{broker_port}");
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
