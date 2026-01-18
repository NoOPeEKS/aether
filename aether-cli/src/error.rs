use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("RedisStorage could not be created.")]
    RedisStorageCreationError,

    #[error("Super user 'admin' already exists.")]
    SuperUserAlreadyExists,

    #[error("Could not create super user 'admin'.")]
    SuperUserCreationError,

    #[error("Redis-based Broker crashed and could not start up.")]
    RedisBrokerCouldNotRun,

    #[error("InMemory-based Broker crashed and could not start up.")]
    InMemoryBrokerCouldNotRun,

    #[error("{0}")]
    BrokerProfileResolveError(String),

    #[error("{0}")]
    ParseTaskError(String),

    #[error("Recieved an unexpected status code: {0}")]
    UnexpectedStatusCode(StatusCode),

    #[error("An error happened while sending the request.")]
    SendRequestError,

    #[error("Could not serialize with serde_json.")]
    SerializeSerdeError,

    #[error("Could not deserialize with serde_json.")]
    DeserializeSerdeError,

    #[error("Could not deserialize request's JSON.")]
    DeserializeRequestError,

    #[error("Could not deserialize the list of Tasks.")]
    DeserializeTaskListError,

    #[error("Could not deserialize the selected Task.")]
    DeserializeTaskError,

    #[error("Could not determine home directory.")]
    InvalidHomeDir,

    #[error("Could not open or generate ~/.aether/config.json. Reason: {0}")]
    GetConfigError(String),

    #[error("Could not save config to ~/.aether/config.json. Reason: {0}")]
    SaveConfigError(String),
}
