use serde::Deserialize;

use crate::{
    Process,
    executors::{local::LocalExecutor, slurm::SlurmExecutor},
};
use std::{io, path::PathBuf};

pub mod local;
pub mod slurm;

pub trait Executor {
    fn exe(&self, script: Process) -> io::Result<Vec<PathBuf>>;
}

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
