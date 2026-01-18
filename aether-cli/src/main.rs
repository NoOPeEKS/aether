mod auth;
mod commands;
mod config;
mod error;
mod task;

use aether_broker::DefaultBroker;
use aether_core::auth::{Permission, User};
use aether_core::broker::storage::{InMemoryStorage, RedisStorage};
use aether_core::capabilities::WorkerCapabilities;
use aether_core::http::{CreateTaskResponse, LoginResponse};
use aether_core::traits::{Broker, Storage};
use aether_worker::Worker;
use bcrypt::{DEFAULT_COST, hash};
use clap::Parser;
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::get_login_jwt;
use crate::commands::{AuthCommands, BrokerCommands, Cli, Commands, TaskCommands, WorkerCommands};
use crate::config::{AetherConfig, BrokerProfile};
use crate::error::CliError;
use crate::task::{cancel_task, check_task, list_tasks, parse_task_file, send_task_to_broker};

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
            } => {
                info!(
                    "[INFO] Starting broker at 0.0.0.0:{api_port} (HTTP API) and 0.0.0.0:{jrpc_port} (JRPC). Listening for connections..."
                );
                let admin_user = User {
                    id: Uuid::new_v4(),
                    name: "admin".into(),
                    password_hash: hash("admin", DEFAULT_COST).expect("To be able to hash."),
                    is_admin: true,
                    permissions: vec![Permission::All],
                };

                if let Some(redis_ip) = redis_ip
                    && let Some(redis_port) = redis_port
                {
                    let storage = RedisStorage::new(&redis_ip, redis_port)
                        .await
                        .map_err(|_| CliError::RedisStorageCreationError)?;

                    storage
                        .create_user(admin_user)
                        .await
                        .map_err(|_| CliError::SuperUserAlreadyExists)?;
                    warn!(
                        "[WARNING] Created default super user 'admin'. Please change its password at `PUT /api/v1/users/admin` !"
                    );
                    let broker = DefaultBroker::new(storage);
                    broker
                        .run(api_port, jrpc_port)
                        .await
                        .map_err(|_| CliError::RedisBrokerCouldNotRun)?;
                } else {
                    let storage = InMemoryStorage::new();
                    storage
                        .create_user(admin_user)
                        .await
                        .map_err(|_| CliError::SuperUserCreationError)?;
                    let broker = DefaultBroker::new(storage);
                    broker
                        .run(api_port, jrpc_port)
                        .await
                        .map_err(|_| CliError::InMemoryBrokerCouldNotRun)?;
                }
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
                    arch: arch.into(),
                };
                let ip = format!("{broker_ip}:{broker_port}");
                let worker = Worker::new(&worker_id, &ip, 10, worker_capabilities);
                let shutdown_token = worker.shutdown_token.clone();
                info!("[INFO] Trying to connect to broker at {broker_ip}:{broker_port}...");
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
                let task_b64 = parse_task_file(&task_file)?;
                let tmp_profile = BrokerProfile::resolve(broker_ip, broker_api_port, token)?;
                let response = send_task_to_broker(
                    &tmp_profile.broker_ip,
                    tmp_profile.broker_api_port,
                    &task_b64,
                    &name,
                    priority,
                    gpu,
                    arch,
                    &tmp_profile
                        .token
                        .expect("A token should have been provided on flag or config file"),
                )
                .await?;
                match response {
                    CreateTaskResponse::Ok { task_id, status } => {
                        println!("Task {} submitted. Status: {:?}", task_id, status);
                    }
                    CreateTaskResponse::Error { message } => {
                        eprintln!("Status Code 500 Internal Server Error: {message}");
                    }
                }
            }
            TaskCommands::Stop {
                broker_ip,
                broker_api_port,
                task_id,
                token,
            } => {
                let tmp_profile = BrokerProfile::resolve(broker_ip, broker_api_port, token)?;
                let response = cancel_task(
                    &tmp_profile.broker_ip,
                    tmp_profile.broker_api_port,
                    &task_id,
                    &tmp_profile
                        .token
                        .expect("A token should have been provided on flag or config profile"),
                )
                .await?;
                println!("{}", response.message);
            }
            TaskCommands::Check {
                broker_ip,
                broker_api_port,
                task_id,
                token,
            } => {
                let tmp_profile = BrokerProfile::resolve(broker_ip, broker_api_port, token)?;
                let response = check_task(
                    &tmp_profile.broker_ip,
                    tmp_profile.broker_api_port,
                    &task_id,
                    &tmp_profile
                        .token
                        .expect("A token should have been provided on flag or config profile."),
                )
                .await?;
                if let Some(err) = response.error {
                    eprintln!("{err}");
                } else if let Some(task) = response.task {
                    let de_task = serde_json::to_string_pretty(&task)
                        .map_err(|_| CliError::DeserializeTaskError)?;
                    println!("{de_task}");
                }
            }
            TaskCommands::List {
                broker_ip,
                broker_api_port,
                token,
            } => {
                let tmp_profile = BrokerProfile::resolve(broker_ip, broker_api_port, token)?;
                let response = list_tasks(
                    &tmp_profile.broker_ip,
                    tmp_profile.broker_api_port,
                    &tmp_profile
                        .token
                        .expect("A token should have been provided on flag or config profile"),
                )
                .await?;
                let de_tasks = serde_json::to_string_pretty(&response)
                    .map_err(|_| CliError::DeserializeTaskListError)?;
                println!("{de_tasks}");
            }
        },
        Commands::Auth { command } => match command {
            AuthCommands::Login {
                profile,
                broker_ip,
                broker_api_port,
                username,
                password,
            } => {
                let resp = get_login_jwt(&broker_ip, broker_api_port, &username, &password).await?;
                match resp {
                    LoginResponse::Ok { jwt } => {
                        let mut cfg = AetherConfig::get()?;
                        let bp = BrokerProfile::new(&broker_ip, broker_api_port, &jwt);
                        cfg.profiles.insert(profile.clone(), bp);
                        cfg.active = Some(profile);
                        cfg.save()?;
                        println!("{jwt}");
                    }
                    LoginResponse::Err { message } => eprintln!("ERROR: {message}"),
                }
            }
            AuthCommands::Switch { profile } => {
                let mut cfg = AetherConfig::get()?;
                if cfg.profiles.contains_key(&profile) {
                    cfg.active = Some(profile.clone());
                    cfg.save()?;
                    println!("Auth profile switched to {profile}.");
                } else {
                    eprintln!("Profile with name `{profile}` does not exist.");
                }
            }
            AuthCommands::Logout { profile } => {
                let mut cfg = AetherConfig::get()?;
                if cfg.profiles.remove(&profile).is_some() {
                    if let Some(ref act) = cfg.active
                        && *act == profile
                    {
                        cfg.active = None;
                    }
                    cfg.save()?;
                    println!("Profile {profile} removed.");
                } else {
                    eprintln!("Profile with name `{profile}` does not exist.");
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
    Ok(())
}
