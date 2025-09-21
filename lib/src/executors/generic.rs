use crate::{
    Process,
    executors::{Executor, local::LocalExecutor, slurm::SlurmExecutor},
};
use serde::Deserialize;
use std::{io, path::PathBuf};

#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
pub enum GenericExecutor {
    Local(LocalExecutor),
    Slurm(Box<SlurmExecutor>),
}

impl GenericExecutor {
    pub fn exe(&self, process: Process) -> io::Result<Vec<PathBuf>> {
        match self {
            GenericExecutor::Local(executor) => executor.exe(process),
            GenericExecutor::Slurm(executor) => executor.exe(process),
        }
    }
}
