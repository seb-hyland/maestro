use crate::{
    Process,
    prelude::{Executor, LocalExecutor, SlurmExecutor},
};
use serde::Deserialize;
use std::{io, path::PathBuf};

#[derive(Clone, Deserialize)]
pub enum GenericExecutor {
    Local(LocalExecutor),
    Slurm(Box<SlurmExecutor>),
}

impl Executor for GenericExecutor {
    fn exe(&self, script: Process) -> io::Result<Vec<PathBuf>> {
        match self {
            Self::Local(executor) => executor.exe(script),
            Self::Slurm(executor) => executor.exe(script),
        }
    }
}
