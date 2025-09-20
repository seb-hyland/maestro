use crate::prelude::{LocalExecutor, SlurmExecutor};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum GenericExecutor {
    Local(LocalExecutor),
    Slurm(Box<SlurmExecutor>),
}
