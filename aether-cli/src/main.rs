use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Aether distributed task queue CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Broker {
        #[command(subcommand)]
        command: BrokerCommands,
    },
    Worker {
        #[command(subcommand)]
        command: WorkerCommands,
    },
    Tui {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_port: usize,
    },
}

#[derive(Subcommand)]
enum BrokerCommands {
    Start {
        #[arg(long)]
        api_port: usize,

        #[arg(long)]
        jrpc_port: usize,
    },
}

#[derive(Subcommand)]
enum WorkerCommands {
    Start {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_port: usize,
    },
}

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
