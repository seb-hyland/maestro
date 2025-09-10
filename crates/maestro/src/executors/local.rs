use crate::{Script, StagingMode, executors::Executor};
use std::{
    fs::read_to_string,
    io::{self, Write as _},
    path::PathBuf,
    process::Command,
};

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
    fn exe(self, mut script: Script) -> io::Result<PathBuf> {
        let (workdir, (log_path, mut log_handle), (launcher_path, mut launcher_handle)) =
            script.prep_script_workdir()?;
        script.stage_inputs(&mut launcher_handle, &workdir, &self.staging_mode)?;
        writeln!(
            launcher_handle,
            "echo -e \":: Launching local process\\nstdout: .maestro.out\\nstderr: .maestro.err\""
        )?;
        writeln!(
            launcher_handle,
            "./.maestro.sh > .maestro.out 2> .maestro.err"
        )?;

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
            Err(io::Error::other(format!(
                "Shell process exited with non-zero exit code. Logs at {}; stderr at {}",
                log_path.display(),
                workdir.join(".maestro.err").display()
            )))
        } else {
            writeln!(
                log_handle,
                ":: Process terminated successfully with exit code 0"
            )?;
            Ok(workdir)
        }
    }
}
