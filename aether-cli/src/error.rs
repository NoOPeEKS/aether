use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("RedisStorage could not be created.")]
    RedisStorageCreation,

    #[error("Super user 'admin' already exists.")]
    SuperUserAlreadyExists,

    #[error("Could not create super user 'admin'.")]
    SuperUserCreation,

    #[error("Redis-based Broker crashed and could not start up.")]
    RedisBrokerCouldNotRun,

    #[error("InMemory-based Broker crashed and could not start up.")]
    InMemoryBrokerCouldNotRun,

    #[error("Could not resolve broker information.")]
    BrokerProfileResolve,

    #[error("{0}")]
    ParseTask(String),

    #[error("Recieved an unexpected status code: {0}")]
    UnexpectedStatusCode(StatusCode),

    #[error("An error happened while sending the request.")]
    SendRequest,

    #[error("Could not deserialize request's JSON.")]
    DeserializeRequest,

    #[error("Could not deserialize the selected Task.")]
    DeserializeTaskList,

    #[error("Could not deserialize the list of Tasks.")]
    DeserializeTask,
}
