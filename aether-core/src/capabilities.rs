use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkerCapabilities {
    pub gpu: bool,
    pub arch: WorkerCPUArchitecture,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum WorkerCPUArchitecture {
    X86_64,
    Aarch64,
}
