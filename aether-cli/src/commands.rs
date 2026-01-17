use aether_core::capabilities::CPUArchitecture;
use aether_core::task::TaskPriority;
use clap::{Parser, Subcommand, ValueEnum};

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
    #[command(about = "Subcommands for handling tasks.")]
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
    #[command(about = "Subcommands for handling broker authentication.")]
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
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

        #[arg(long, requires = "redis_port")]
        redis_ip: Option<String>,

        #[arg(long, requires = "redis_ip")]
        redis_port: Option<usize>,
    },
}

#[derive(Subcommand)]
pub enum WorkerCommands {
    #[command(about = "Starts an Aether worker that connects to an existing broker.")]
    Start {
        #[arg(long)]
        worker_id: String,

        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_port: usize,

        #[arg(long)]
        gpu: bool,

        #[arg(value_enum, long)]
        arch: SupportedArchs,
    },
}

#[derive(Subcommand)]
pub enum TaskCommands {
    Submit {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_api_port: usize,

        #[arg(long)]
        task_file: String,

        #[arg(long)]
        name: String,

        #[arg(value_enum, long, default_value_t = SupportedPriorities::Medium)]
        priority: SupportedPriorities,

        #[arg(long, default_value_t = false)]
        gpu: bool,

        #[arg(value_enum, long, default_value_t = SupportedArchs::X86_64)]
        arch: SupportedArchs,
    },
    Stop {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_api_port: usize,

        #[arg(long)]
        task_id: String,
    },
    Check {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_api_port: usize,

        #[arg(long)]
        task_id: String,
    },
    List {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_api_port: usize,
    },
}

#[derive(Clone, ValueEnum)]
pub enum SupportedArchs {
    X86_64,
    Aarch64,
    Any,
}

impl From<SupportedArchs> for CPUArchitecture {
    fn from(value: SupportedArchs) -> Self {
        match value {
            SupportedArchs::X86_64 => CPUArchitecture::X86_64,
            SupportedArchs::Aarch64 => CPUArchitecture::Aarch64,
            SupportedArchs::Any => CPUArchitecture::Any,
        }
    }
}

#[derive(Clone, ValueEnum)]
pub enum SupportedPriorities {
    High,
    Medium,
    Low,
}

impl From<SupportedPriorities> for TaskPriority {
    fn from(value: SupportedPriorities) -> Self {
        match value {
            SupportedPriorities::High => TaskPriority::High,
            SupportedPriorities::Medium => TaskPriority::Medium,
            SupportedPriorities::Low => TaskPriority::Low,
        }
    }
}

#[derive(Subcommand)]
pub enum AuthCommands {
    #[command(about = "Log in with the provided credentials and save the JWT token.")]
    Login {
        #[arg(long)]
        broker_ip: String,

        #[arg(long)]
        broker_api_port: usize,

        #[arg(long)]
        username: String,

        #[arg(long)]
        password: String,
    },
}
