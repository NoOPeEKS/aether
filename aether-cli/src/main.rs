mod auth;
mod commands;
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
use crate::task::{cancel_task, check_task, list_tasks, parse_task_file, send_task_to_broker};

#[tokio::main]
async fn main() {
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
                warn!(
                    "[WARNING] Created default super user 'admin'. Please change its password at `PUT /api/v1/users/admin` !"
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
                        .expect("RedisStorage should have been created.");
                    storage
                        .create_user(admin_user)
                        .await
                        .expect("To be able to create admin user.");
                    let broker = DefaultBroker::new(storage);
                    broker
                        .run(api_port, jrpc_port)
                        .await
                        .expect("Broker should run");
                } else {
                    let storage = InMemoryStorage::new();
                    storage
                        .create_user(admin_user)
                        .await
                        .expect("To be able to create admin user.");
                    let broker = DefaultBroker::new(storage);
                    broker
                        .run(api_port, jrpc_port)
                        .await
                        .expect("Broker should run");
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
            } => {
                let task_b64 = match parse_task_file(&task_file) {
                    Ok(task_b64) => task_b64,
                    Err(err) => {
                        eprintln!("ERROR: {err}");
                        return;
                    }
                };
                match send_task_to_broker(
                    &broker_ip,
                    broker_api_port,
                    &task_b64,
                    &name,
                    priority,
                    gpu,
                    arch,
                )
                .await
                {
                    Ok(response) => match response {
                        CreateTaskResponse::Ok { task_id, status } => {
                            println!("Task {} submitted. Status: {:?}", task_id, status);
                        }
                        CreateTaskResponse::Error { message } => {
                            eprintln!("Status Code 500 Internal Server Error: {message}");
                        }
                    },
                    Err(err) => {
                        eprintln!("ERROR: {err}");
                    }
                }
            }
            TaskCommands::Stop {
                broker_ip,
                broker_api_port,
                task_id,
            } => match cancel_task(&broker_ip, broker_api_port, &task_id).await {
                Ok(resp) => {
                    println!("{}", resp.message);
                }
                Err(err) => eprintln!("ERROR: {err}"),
            },
            TaskCommands::Check {
                broker_ip,
                broker_api_port,
                task_id,
            } => match check_task(&broker_ip, broker_api_port, &task_id).await {
                Ok(resp) => {
                    if let Some(err) = resp.error {
                        eprintln!("{err}");
                    } else if let Some(task) = resp.task {
                        let de_task = serde_json::to_string_pretty(&task)
                            .expect("Failed deserialization of task.");
                        println!("{de_task}");
                    }
                }
                Err(err) => eprintln!("ERROR: {err}"),
            },
            TaskCommands::List {
                broker_ip,
                broker_api_port,
            } => match list_tasks(&broker_ip, broker_api_port).await {
                Ok(resp) => {
                    let de_tasks = serde_json::to_string_pretty(&resp)
                        .expect("Failed deserialization of all tasks");
                    println!("{de_tasks}");
                }
                Err(err) => eprintln!("ERROR: {err}"),
            },
        },
        Commands::Auth { command } => match command {
            AuthCommands::Login {
                broker_ip,
                broker_api_port,
                username,
                password,
            } => match get_login_jwt(&broker_ip, broker_api_port, &username, &password).await {
                Ok(resp) => match resp {
                    LoginResponse::Ok { jwt } => println!("{jwt}"),
                    LoginResponse::Err { message } => eprintln!("ERROR: {message}"),
                },
                Err(err) => eprintln!("ERROR: {err}"),
            },
        },
        Commands::Tui {
            broker_ip: _,
            broker_port: _,
        } => {
            println!("In the future, TUI will execute here.");
        }
    }
}
