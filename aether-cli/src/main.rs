mod auth;
mod commands;
mod config;
mod error;
mod handlers;
mod task;
mod tui;

use clap::Parser;

use crate::commands::{AuthCommands, BrokerCommands, Cli, Commands, TaskCommands, WorkerCommands};
use crate::error::CliError;
use crate::handlers::{
    handle_auth_login, handle_auth_logout, handle_auth_switch, handle_broker_start,
    handle_task_check, handle_task_list, handle_task_stop, handle_task_submit, handle_worker_start,
};
use crate::tui::run_tui;

#[tokio::main]
async fn main() -> Result<(), CliError> {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Broker { command } => match command {
            BrokerCommands::Start {
                api_port,
                jrpc_port,
                redis_ip,
                redis_port,
            } => handle_broker_start(api_port, jrpc_port, redis_ip, redis_port).await?,
        },
        Commands::Worker { command } => match command {
            WorkerCommands::Start {
                worker_id,
                broker_ip,
                broker_port,
                gpu,
                arch,
            } => handle_worker_start(worker_id, broker_ip, broker_port, gpu, arch).await?,
        },
        Commands::Task { command } => match command {
            TaskCommands::Submit {
                broker_ip,
                broker_api_port,
                task_file,
                name,
                priority,
                gpu,
                arch,
                token,
            } => {
                handle_task_submit(
                    broker_ip,
                    broker_api_port,
                    task_file,
                    name,
                    priority,
                    gpu,
                    arch,
                    token,
                )
                .await?
            }
            TaskCommands::Stop {
                broker_ip,
                broker_api_port,
                task_id,
                token,
            } => handle_task_stop(broker_ip, broker_api_port, task_id, token).await?,
            TaskCommands::Check {
                broker_ip,
                broker_api_port,
                task_id,
                token,
            } => handle_task_check(broker_ip, broker_api_port, task_id, token).await?,
            TaskCommands::List {
                broker_ip,
                broker_api_port,
                token,
            } => handle_task_list(broker_ip, broker_api_port, token).await?,
        },
        Commands::Auth { command } => match command {
            AuthCommands::Login {
                profile,
                broker_ip,
                broker_api_port,
                username,
                password,
            } => handle_auth_login(profile, broker_ip, broker_api_port, username, password).await?,
            AuthCommands::Switch { profile } => handle_auth_switch(profile).await?,
            AuthCommands::Logout { profile } => handle_auth_logout(profile).await?,
        },
        Commands::Tui {
            broker_ip: _,
            broker_port: _,
        } => {
            run_tui().await.map_err(|_| CliError::TuiError)?;
        }
    }
    Ok(())
}
