use std::{
    borrow::Cow,
    fmt::Display,
    fs::{File, OpenOptions, create_dir_all},
    io::{self, Write as _},
    ops::Not,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

use crate::session::SESSION_WORKDIR;

pub mod executors;
mod macros;
pub mod prelude;
mod session;

pub type PathArg = (Cow<'static, str>, PathBuf);
pub type StrArg = (Cow<'static, str>, String);

#[derive(Clone)]
pub struct Process {
    name: String,
    script: Cow<'static, str>,
    inputs: Vec<PathArg>,
    outputs: Vec<PathArg>,
    args: Vec<StrArg>,
}

type PathAndHandle = (PathBuf, File);
impl Process {
    pub fn new(
        name: String,
        script: Cow<'static, str>,
        inputs: Vec<PathArg>,
        outputs: Vec<PathArg>,
        args: Vec<StrArg>,
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
    ) -> Result<(PathBuf, PathAndHandle, PathAndHandle), io::Error> {
        let process_workdir = {
            let session_dir = SESSION_WORKDIR
                .as_ref()
                .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
            let dir = session_dir.join(&self.name);
            if dir.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Process working directory {} already exists! Use a unique process name to avoid collisions",
                        dir.display()
                    ),
                ));
            }
            create_dir_all(&dir)?;
            dir
        };

        let script_path = process_workdir.join(".maestro.sh");
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&script_path)?;
        script_file.write_all(self.script.as_bytes())?;

        let log_path = process_workdir.join(".maestro.log");
        let log_handle = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&log_path)?;

        let launcher_path = process_workdir.join(".maestro.launcher");
        let mut launcher_handle = OpenOptions::new()
            .append(true)
            .create_new(true)
            .mode(0o755)
            .open(&launcher_path)?;
        writeln!(launcher_handle, "#!/bin/bash")?;

        Ok((
            process_workdir,
            (log_path, log_handle),
            (launcher_path, launcher_handle),
        ))
    }

    fn stage_inputs(
        &self,
        launcher: &mut File,
        workdir: &Path,
        staging_mode: &StagingMode,
    ) -> io::Result<()> {
        writeln!(launcher, "set -euo pipefail")?;

        let input_dir = PathBuf::from("maestro_inputs/");
        let stage_inputs = matches!(staging_mode, StagingMode::None).not();
        writeln!(
            launcher,
            "echo \":: Process workdir initialized at {}\"\necho \":: Staging inputs to {}\"",
            workdir.display(),
            input_dir.display()
        )?;
        writeln!(launcher, "mkdir {}", input_dir.display())?;

        self.check_files(CheckTime::Input, None)?;

        for (var, file) in &self.inputs {
            let var = var.split_whitespace().collect::<Vec<_>>().join("_");
            let transformed_arg = if stage_inputs {
                let file_name = file.file_name().ok_or(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Could not resolve file name of {}", file.display()),
                ))?;
                let destination = input_dir.join(format!("[{}]{}", var, file_name.display()));
                destination.to_string_lossy().into_owned()
            } else {
                file.canonicalize()?.to_string_lossy().into_owned()
            };
            writeln!(launcher, "export {}=\"{}\"", var, transformed_arg)?;
            if stage_inputs {
                writeln!(
                    launcher,
                    "{} \"{}\" \"${}\"",
                    staging_mode,
                    file.canonicalize()?.display(),
                    var
                )?;
            }
        }

        for (var, arg) in &self.outputs {
            writeln!(launcher, "export {var}=\"{}\"", arg.display())?;
        }
        for (var, arg) in &self.args {
            writeln!(launcher, "export {var}=\"{arg}\"")?;
        }
        Ok(())
    }

    fn check_files(&self, time: CheckTime, maybe_dir: Option<&Path>) -> io::Result<()> {
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
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Expected {} files for process {} do not exist: [{}]",
                    check_time, self.name, file_names
                ),
            ));
        }

        Ok(())
    }

    fn write_execution(mut launcher_handle: File, error_handling: bool) -> io::Result<()> {
        writeln!(
            launcher_handle,
            "{}./.maestro.sh >> .maestro.out 2>> .maestro.err",
            if error_handling { "source " } else { "" }
        )
    }
}

enum CheckTime {
    Input,
    Output,
}

#[derive(Clone, Copy)]
pub enum StagingMode {
    Copy,
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

const LP: &str = "\x1b[0;34m::\x1b[0m";
