use crate::{Container, PathArg, Process, StrArg, session::SESSION_WORKDIR};
use dagger_lib::result::{NodeError, NodeResult};
use serde::Deserialize;
use std::{
    borrow::Cow,
    fmt::Display,
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

type PathAndHandle = (PathBuf, File);
impl Process {
    /// Constructs a new process
    pub fn new(
        name: String,
        inputs: Vec<PathArg>,
        args: Vec<StrArg>,
        outputs: Vec<PathArg>,
        script: Cow<'static, str>,
    ) -> Self {
        Process {
            name,
            script,
            inputs,
            outputs,
            args,
        }
    }

    pub(crate) fn prep_script_workdir(
        &mut self,
    ) -> NodeResult<(PathBuf, PathAndHandle, PathAndHandle)> {
        // Initialized in maestro::initialize
        let session_dir = SESSION_WORKDIR.get().unwrap().to_path_buf();

        let dir = session_dir.join(&self.name);
        if dir.exists() {
            return Err(NodeError::msg(format!(
                "Process working directory {} already exists! Use a unique process name to avoid collisions",
                dir.display()
            )));
        }
        fs::create_dir(&dir).map_err(|e| {
            NodeError::msg(format!("Failed to spawn process working directory at {e}"))
        })?;

        let script_path = dir.join(".maestro.sh");
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&script_path)
            .map_err(|e| NodeError::msg(format!("Failed to open script path: {e}")))?;
        script_file
            .write_all(self.script.as_bytes())
            .map_err(|e| NodeError::msg(format!("Failed to write to script file: {e}")))?;

        let log_path = dir.join(".maestro.log");
        let log_handle = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| NodeError::msg(format!("Failed to open log file: {e}")))?;

        let launcher_path = dir.join(".maestro.launcher");
        let mut launcher_handle = OpenOptions::new()
            .append(true)
            .create_new(true)
            .mode(0o755)
            .open(&launcher_path)
            .map_err(|e| NodeError::msg(format!("Failed to open launcher file: {e}")))?;
        writeln!(launcher_handle, "#!/bin/bash")
            .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;

        Ok((
            dir,
            (log_path, log_handle),
            (launcher_path, launcher_handle),
        ))
    }

    pub(crate) fn stage_inputs(
        &self,
        launcher: &mut File,
        workdir: &Path,
        staging_mode: &StagingMode,
    ) -> NodeResult<()> {
        writeln!(launcher, "set -euo pipefail")
            .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;

        let input_dir = Path::new("maestro_inputs/");
        writeln!(
            launcher,
            "echo \":: Process workdir initialized at {}\"\necho \":: Staging inputs to {}\"",
            workdir.display(),
            input_dir.display()
        )
        .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        writeln!(launcher, "mkdir {}", input_dir.display())
            .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;

        self.check_files(CheckTime::Input, None)?;

        let stage_inputs = !matches!(staging_mode, StagingMode::None);
        for (var, file) in &self.inputs {
            let var = var.split_whitespace().collect::<Vec<_>>().join("_");
            let transformed_arg = if stage_inputs {
                let file_name = file.file_name().ok_or(NodeError::msg(format!(
                    "Could not resolve file name of input {}",
                    file.display()
                )))?;
                let destination = input_dir.join(format!("[{}]{}", var, file_name.display()));
                destination.to_string_lossy().into_owned()
            } else {
                file.canonicalize()?.to_string_lossy().into_owned()
            };
            writeln!(launcher, "export {}=\"{}\"", var, transformed_arg)
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
            if stage_inputs {
                writeln!(
                    launcher,
                    "{} \"{}\" \"${}\"",
                    staging_mode,
                    file.canonicalize()?.display(),
                    var
                )
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
            }
        }

        for (var, arg) in &self.outputs {
            writeln!(launcher, "export {var}=\"{}\"", arg.display())
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        }
        for (var, arg) in &self.args {
            writeln!(launcher, "export {var}=\"{arg}\"")
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        }
        Ok(())
    }

    pub(crate) fn check_files(&self, time: CheckTime, maybe_dir: Option<&Path>) -> NodeResult<()> {
        let files = match time {
            CheckTime::Input => &self.inputs,
            CheckTime::Output => &self.outputs,
        };
        let non_existent_files: Vec<_> = files
            .iter()
            .filter_map(|(_, p)| {
                let path = if let Some(dir) = maybe_dir {
                    dir.join(p)
                } else {
                    p.clone()
                };
                if path.exists() { None } else { Some(path) }
            })
            .collect();
        // Some non-existent file
        if !non_existent_files.is_empty() {
            let file_names = non_existent_files
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let check_time = match time {
                CheckTime::Input => "input",
                CheckTime::Output => "output",
            };
            return Err(NodeError::msg(format!(
                "Expected {} files for process {} do not exist: [{}]",
                check_time, self.name, file_names
            )));
        }

        Ok(())
    }

    pub(crate) fn write_execution(
        mut launcher_handle: File,
        process: &Process,
        container: &Option<Container>,
    ) -> NodeResult<()> {
        let execution_str = "./.maestro.sh >> .maestro.out 2>> .maestro.err";
        let image = match container {
            None => {
                return writeln!(launcher_handle, "{execution_str}")
                    .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")));
            }
            Some(runtime) => match runtime {
                Container::Docker(image) | Container::Podman(image) => {
                    let binary = match runtime {
                        Container::Docker(_) => "docker",
                        Container::Podman(_) => "podman",
                        _ => unreachable!(),
                    };
                    write!(
                        launcher_handle,
                        "{binary} run --rm -v $(pwd):/maestro -w /maestro "
                    )
                    .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
                    image
                }
                Container::Apptainer(image) => {
                    write!(
                        launcher_handle,
                        "apptainer exec --bind .:/maestro --workdir /maestro "
                    )
                    .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
                    image
                }
            },
        };
        for input in &process.inputs {
            write!(launcher_handle, "-e {} ", input.0)
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        }
        for arg in &process.args {
            write!(launcher_handle, "-e {} ", arg.0)
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        }
        for output in &process.outputs {
            write!(launcher_handle, "-e {} ", output.0)
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        }
        writeln!(launcher_handle, "{image} bash -c \"{execution_str}\"")
            .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))
    }
}

pub(crate) enum CheckTime {
    Input,
    Output,
}

#[derive(Clone, Copy, Deserialize, Default)]
pub enum StagingMode {
    Copy,
    #[default]
    Symlink,
    None,
}
impl Display for StagingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy => write!(f, "cp -r"),
            Self::Symlink => write!(f, "ln -s"),
            Self::None => write!(f, ""),
        }
    }
}
