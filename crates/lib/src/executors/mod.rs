use crate::Process;
use std::{io, path::PathBuf};

pub mod local;
pub mod slurm;

pub trait Executor {
    fn exe<'a>(self, script: Process<'a>) -> io::Result<Vec<PathBuf>>;
}
