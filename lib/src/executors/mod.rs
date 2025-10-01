use serde::Deserialize;

use crate::{
    Process, WorkflowResult,
    executors::{local::LocalExecutor, slurm::SlurmExecutor},
};

pub mod local;
pub mod slurm;

pub trait Executor {
    fn exe(&self, script: Process) -> WorkflowResult;
}

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
