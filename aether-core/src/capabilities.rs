use serde::{Deserialize, Serialize};

use crate::task::{TaskArchitecture, TaskCapabilities};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkerCapabilities {
    pub gpu: bool,
    pub arch: WorkerCPUArchitecture,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum WorkerCPUArchitecture {
    X86_64,
    Aarch64,
    Any,
}

impl WorkerCapabilities {
    pub fn supports(&self, task_caps: Option<TaskCapabilities>) -> bool {
        if let Some(task_caps) = task_caps {
            if !self.gpu && task_caps.gpu {
                return false;
            }
            match self.arch {
                WorkerCPUArchitecture::X86_64 => {
                    task_caps.arch == TaskArchitecture::X86_64
                        || task_caps.arch == TaskArchitecture::Any
                }
                WorkerCPUArchitecture::Aarch64 => {
                    task_caps.arch == TaskArchitecture::Aarch64
                        || task_caps.arch == TaskArchitecture::Any
                }
                WorkerCPUArchitecture::Any => true,
            }
        } else {
            true
        }
    }
}
