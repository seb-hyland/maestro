use crate::{CheckTime, LP, Process, StagingMode, executors::Executor};
use std::{
    io::{self, Write as _},
    path::PathBuf,
    process::Command,
};

#[derive(Clone, Copy)]
pub struct LocalExecutor {
    staging_mode: StagingMode,
}

impl Default for LocalExecutor {
    fn default() -> Self {
        Self {
            staging_mode: StagingMode::Symlink,
        }
    }
}

impl LocalExecutor {
    pub fn with_staging_mode(mut self, mode: StagingMode) -> Self {
        self.staging_mode = mode;
        self
    }
}

impl Executor for LocalExecutor {
    fn exe(&self, mut process: Process) -> io::Result<Vec<PathBuf>> {
        let (workdir, (log_path, mut log_handle), (launcher_path, mut launcher_handle)) =
            process.prep_script_workdir()?;
        let staging_mode = match process.container {
            None => &self.staging_mode,
            Some(_) => &StagingMode::Copy,
        };
        process.stage_inputs(&mut launcher_handle, &workdir, staging_mode)?;
        writeln!(
            launcher_handle,
            "echo -e \":: Launching local process\\nstdout: .maestro.out\\nstderr: .maestro.err\""
        )?;
        Process::write_execution(launcher_handle, &process)?;

        let output = Command::new(launcher_path)
            .stdout(log_handle.try_clone()?)
            .stderr(log_handle.try_clone()?)
            .current_dir(&workdir)
            .output()?;

        if !output.status.success() {
            writeln!(log_handle, "{LP} Process failed!")?;
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
