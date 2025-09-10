use std::{
    fmt::Display,
    fs::{File, OpenOptions},
    io::{self, Write as _},
    ops::Not,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

use crate::session::create_process_dir;

pub mod executors;
mod macros;
mod session;

pub struct Script<'a> {
    pub script: &'a str,
    pub vars: &'a mut [(&'a str, Injection)],
}

type FilePair = (PathBuf, File);
impl<'a> Script<'a> {
    pub(crate) fn prep_script_workdir(
        &mut self,
    ) -> Result<(PathBuf, FilePair, FilePair), io::Error> {
        let process_workdir = create_process_dir()?;

        let script_path = process_workdir.join("maestro.sh");
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&script_path)?;
        script_file.write_all(self.script.as_bytes())?;

        let log_path = process_workdir.join("maestro.log");
        let log_handle = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&log_path)?;

        let launcher_path = process_workdir.join("maestro.launcher");
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
        let input_dir = PathBuf::from("maestro_inputs/");
        writeln!(
            launcher,
            "echo \":: Process workdir initialized at {}\"\necho \":: Staging inputs to {}\"",
            workdir.display(),
            input_dir.display()
        )?;
        writeln!(launcher, "mkdir {}", input_dir.display())?;
        for (var, arg) in self.vars.iter() {
            let var = var.split_whitespace().collect::<Vec<_>>().join("_");
            let transformed_arg = match arg {
                Injection::File(f) => {
                    if f.exists() {
                        let file_name = f.file_name().ok_or(io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("Could not resolve file name of {}", f.display()),
                        ))?;
                        let destination =
                            input_dir.join(format!("[{}]{}", var, file_name.display()));
                        &destination.to_string_lossy().into_owned()
                    } else {
                        &f.to_string_lossy().into_owned()
                    }
                }
                Injection::Param(p) => p,
            };
            writeln!(launcher, "export {}=\"{}\"", var, transformed_arg)?;
            if let Injection::File(f) = arg
                && f.exists()
                && matches!(staging_mode, StagingMode::None).not()
            {
                writeln!(
                    launcher,
                    "{} \"{}\" \"${}\"",
                    staging_mode,
                    f.canonicalize()?.display(),
                    var
                )?;
            }
        }
        Ok(())
    }
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

pub enum Injection {
    Param(String),
    File(PathBuf),
}
impl From<String> for Injection {
    fn from(s: String) -> Self {
        Self::Param(s)
    }
}
impl<'a> From<&'a str> for Injection {
    fn from(s: &'a str) -> Self {
        Self::Param(s.to_string())
    }
}
impl From<PathBuf> for Injection {
    fn from(p: PathBuf) -> Self {
        Self::File(p)
    }
}
impl<'a> From<&'a Path> for Injection {
    fn from(p: &'a Path) -> Self {
        Self::File(p.to_path_buf())
    }
}

pub trait OutputMapper {
    fn join_outputs<const N: usize>(&self, paths: [&Path; N]) -> Vec<PathBuf>;
}
impl OutputMapper for PathBuf {
    fn join_outputs<const N: usize>(&self, paths: [&Path; N]) -> Vec<PathBuf> {
        paths.iter().map(|p| self.join(p)).collect()
    }
}

pub trait OutputChecker {
    fn check_path<'a>(&'a self, vec: &mut Vec<&'a Path>);
}
fn inner<'a, P: AsRef<Path> + ?Sized>(path: &'a P, target: &mut Vec<&'a Path>) {
    let path = path.as_ref();
    if !path.exists() {
        target.push(path)
    }
}
impl OutputChecker for Path {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        inner(self, target);
    }
}
impl OutputChecker for PathBuf {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        inner(self, target);
    }
}
impl OutputChecker for Vec<PathBuf> {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        for path in self {
            inner(path, target);
        }
    }
}
impl OutputChecker for Vec<&Path> {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        for path in self {
            inner(path, target);
        }
    }
}
impl<const N: usize> OutputChecker for [PathBuf; N] {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        for path in self.iter() {
            inner(path, target);
        }
    }
}
impl<const N: usize> OutputChecker for [&Path; N] {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        for path in self.iter() {
            inner(path, target);
        }
    }
}
impl OutputChecker for &[PathBuf] {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        for path in self.iter() {
            inner(path, target);
        }
    }
}
impl OutputChecker for &[&Path] {
    fn check_path<'a>(&'a self, target: &mut Vec<&'a Path>) {
        for path in self.iter() {
            inner(path, target);
        }
    }
}
