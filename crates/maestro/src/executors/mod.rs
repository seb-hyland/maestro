use crate::Script;
use std::{io, path::PathBuf};

pub mod local;
pub mod slurm;

pub trait Executor {
    fn exe(self, script: Script) -> io::Result<PathBuf>;
}
