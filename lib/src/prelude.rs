pub use crate::{
    StagingMode,
    executors::{
        Executor,
        local::LocalExecutor,
        slurm::{MailType, Memory, MemoryConfig, SlurmConfig, SlurmExecutor, SlurmTime},
    },
};
pub use maestro_macros::process;
