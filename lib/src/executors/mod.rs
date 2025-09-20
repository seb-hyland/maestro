use crate::Process;
use std::{io, path::PathBuf};

pub mod generic;
pub mod local;
pub mod slurm;

pub trait Executor {
    fn exe(&self, script: Process) -> io::Result<Vec<PathBuf>>;
}
