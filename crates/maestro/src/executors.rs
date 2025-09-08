use std::{
    fs::{File, read_to_string},
    io::{self, Write as _},
    process::Command,
};

use crate::{Script, workflow::CopyMode};

pub trait Executor {
    fn exe(self, script: Script) -> io::Result<()>;
}

pub struct LocalExecutor {
    copy_mode: CopyMode,
    create_parents: bool,
}

impl Default for LocalExecutor {
    fn default() -> Self {
        Self {
            copy_mode: CopyMode::Copy,
            create_parents: true,
        }
    }
}

impl LocalExecutor {
    pub fn with_copy_mode(mut self, mode: CopyMode) -> Self {
        self.copy_mode = mode;
        self
    }
    pub fn create_parents(mut self, yes: bool) -> Self {
        self.create_parents = yes;
        self
    }
}

impl Executor for LocalExecutor {
    fn exe(self, mut script: Script) -> io::Result<()> {
        let (workdir, script_path, log_path, mut log_handle) =
            script.prep_script_inputs(self.copy_mode)?;
        let vars = script.display_vars();

        // TODO! Chrono
        writeln!(log_handle, ":: Spawning process script")?;
        writeln!(log_handle, ":: Logging process output...")?;
        let log_stderr_path = workdir.join(".maestro.err");
        let output = Command::new(script_path)
            .stdout(log_handle.try_clone()?)
            .stderr(File::create(&log_stderr_path)?)
            .envs(vars)
            .current_dir(workdir)
            .output()?;

        if !output.status.success() {
            writeln!(log_handle, ":: Process failed!")?;
            if let Some(exit_code) = output.status.code() {
                writeln!(log_handle, ":: Exit code: {exit_code}")?;
            }
            let stderr = match read_to_string(&log_stderr_path) {
                Ok(stderr) => stderr,
                Err(e) => format!("Failed to read .maestro.err: {e:?}"),
            };
            writeln!(log_handle, ":: Process stderr:\n{stderr}")?;
            Err(io::Error::other(format!(
                "Shell process exited with non-zero exit code. Logs at {}; stderr at {}",
                log_path.display(),
                log_stderr_path.display()
            )))
        } else {
            writeln!(log_handle, ":: Process terminated successfully!")?;
            Ok(())
        }
    }
}
