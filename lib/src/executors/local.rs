use crate::{
    Container, LP, Process,
    executors::Executor,
    process::{CheckTime, StagingMode},
};
use dagger_lib::result::{NodeError, NodeResult};
use serde::Deserialize;
use std::{io::Write as _, path::PathBuf, process::Command};

#[derive(Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LocalExecutor {
    #[serde(default)]
    pub(crate) staging_mode: StagingMode,
    pub(crate) container: Option<Container>,
}

impl LocalExecutor {
    pub fn with_staging_mode(mut self, mode: StagingMode) -> Self {
        self.staging_mode = mode;
        self
    }
}

impl Executor for LocalExecutor {
    fn exe(&self, mut process: Process) -> NodeResult<Vec<PathBuf>> {
        let (workdir, (log_path, mut log_handle), (launcher_path, mut launcher_handle)) =
            process.prep_script_workdir()?;
        let staging_mode = match self.container {
            None => &self.staging_mode,
            Some(_) => &StagingMode::Copy,
        };
        process.stage_inputs(&mut launcher_handle, &workdir, staging_mode)?;
        writeln!(
            launcher_handle,
            "echo -e \":: Launching local process\\nstdout: .maestro.out\\nstderr: .maestro.err\""
        )
        .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        Process::write_execution(launcher_handle, &process, &self.container)?;

        let output = Command::new(launcher_path)
            .stdout(log_handle.try_clone()?)
            .stderr(log_handle.try_clone()?)
            .current_dir(&workdir)
            .output()
            .map_err(|e| NodeError::msg(format!("Failed to spawn launcher process: {e}")))?;

        if !output.status.success() {
            let _ = writeln!(log_handle, "{LP} Process failed!");
            if let Some(exit_code) = output.status.code() {
                let _ = writeln!(log_handle, "Exit code: {exit_code}");
            }
            let _ = writeln!(log_handle, "stderr at .maestro.err");
            return Err(NodeError::msg(format!(
                "Shell process exited with non-zero exit code. Logs at {}; stderr at {}",
                log_path.display(),
                workdir.join(".maestro.err").display()
            )));
        } else {
            let _ = writeln!(
                log_handle,
                ":: Process terminated successfully with exit code 0"
            );
        }

        process.check_files(CheckTime::Output, Some(&workdir))?;
        let mut outputs: Vec<_> = process
            .outputs
            .iter()
            .map(|(_, p)| workdir.join(p))
            .collect();
        outputs.push(workdir);
        Ok(outputs)
    }
}
