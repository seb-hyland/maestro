use serde::Deserialize;

use crate::{
    Process, WorkflowResult,
    executors::{local::LocalExecutor, slurm::SlurmExecutor},
};

/// Local execution
pub mod local;
/// Slurm execution
pub mod slurm;

/// Generic trait to implement executors against
pub trait Executor {
    fn exe(&self, script: Process) -> WorkflowResult;
}

/// Generic executor enum for deserializing executor definitions from Maestro.toml
#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
pub enum GenericExecutor {
    Local(LocalExecutor),
    Slurm(Box<SlurmExecutor>),
}

impl GenericExecutor {
    pub fn exe(&self, process: Process) -> WorkflowResult {
        match self {
            GenericExecutor::Local(executor) => executor.exe(process),
            GenericExecutor::Slurm(executor) => executor.exe(process),
        }
    }
}
