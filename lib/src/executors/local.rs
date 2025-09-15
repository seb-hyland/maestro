use crate::{CheckTime, Process, StagingMode, executors::Executor};
use std::{
    io::{self, Write as _},
    path::PathBuf,
    process::Command,
};

pub struct LocalExecutor {
    staging_mode: StagingMode,
    error_handling: bool,
}

impl Default for LocalExecutor {
    fn default() -> Self {
        Self {
            staging_mode: StagingMode::Symlink,
            error_handling: true,
        }
    }
}

impl LocalExecutor {
    pub fn with_staging_mode(mut self, mode: StagingMode) -> Self {
        self.staging_mode = mode;
        self
    }
    pub fn with_error_handling(mut self, y: bool) -> Self {
        self.error_handling = y;
        self
    }
}

impl Executor for LocalExecutor {
    fn exe(self, mut process: Process) -> io::Result<Vec<PathBuf>> {
        let (workdir, (log_path, mut log_handle), (launcher_path, mut launcher_handle)) =
            process.prep_script_workdir()?;
        process.stage_inputs(&mut launcher_handle, &workdir, &self.staging_mode)?;
        writeln!(
            launcher_handle,
            "echo -e \":: Launching local process\\nstdout: .maestro.out\\nstderr: .maestro.err\""
        )?;
        Process::write_execution(launcher_handle, self.error_handling)?;

        let output = Command::new(launcher_path)
            .stdout(log_handle.try_clone()?)
            .stderr(log_handle.try_clone()?)
            .current_dir(&workdir)
            .output()?;

        if !output.status.success() {
            writeln!(log_handle, ":: Process failed!")?;
            if let Some(exit_code) = output.status.code() {
                writeln!(log_handle, "Exit code: {exit_code}")?;
            }
            writeln!(log_handle, "stderr at .maestro.err")?;
            return Err(io::Error::other(format!(
                "Shell process exited with non-zero exit code. Logs at {}; stderr at {}",
                log_path.display(),
                workdir.join(".maestro.err").display()
            )));
        } else {
            writeln!(
                log_handle,
                ":: Process terminated successfully with exit code 0"
            )?;
        }

        process.check_files(CheckTime::Output, Some(&workdir))?;
        let mut outputs: Vec<_> = process
            .outputs
            .iter()
            .map(|(_, p)| workdir.join(p))
            .collect();
        outputs.insert(0, workdir);
        Ok(outputs)
    }
}
