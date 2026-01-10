use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkerCapabilities {
    pub gpu: bool,
    pub arch: CPUArchitecture,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskCapabilities {
    pub gpu: bool,
    pub arch: CPUArchitecture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum CPUArchitecture {
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
                CPUArchitecture::X86_64 => {
                    task_caps.arch == CPUArchitecture::X86_64
                        || task_caps.arch == CPUArchitecture::Any
                }
                CPUArchitecture::Aarch64 => {
                    task_caps.arch == CPUArchitecture::Aarch64
                        || task_caps.arch == CPUArchitecture::Any
                }
                CPUArchitecture::Any => true,
            }
        } else {
            true
        }
    }
}
