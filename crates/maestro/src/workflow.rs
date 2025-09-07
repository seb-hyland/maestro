use crate::{Injection, Script, session::create_process_dir};
use std::{
    fs::{self, File, OpenOptions, create_dir},
    io::{self, Write},
    os::{self, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
};

pub enum CopyMode {
    Copy,
    Symlink,
}

impl<'a> Script<'a> {
    fn prep_script_inputs(
        &mut self,
        handler: fn(&Path, &Path) -> io::Result<()>,
    ) -> Result<(PathBuf, PathBuf, PathBuf), io::Error> {
        let process_workdir = create_process_dir()?;
        let input_dir = process_workdir.join(".maestro_inputs");
        create_dir(&input_dir)?;

        for (_, injection) in self.vars.iter_mut() {
            if let Injection::File(origin_path) = injection {
                let name = origin_path.file_name().ok_or(io::Error::new(
                    io::ErrorKind::InvalidFilename,
                    format!("No file name for path {origin_path:?}"),
                ))?;
                let destination = input_dir.join(name);
                handler(origin_path.canonicalize()?.as_path(), &destination)?;
                *injection = Injection::File(destination)
            }
        }

        let script_path = input_dir.join(".maestro.sh");
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&script_path)?;
        script_file.write_all(self.script.as_bytes())?;

        Ok((process_workdir, input_dir, script_path))
    }

    fn recursive_walker(&mut self, base_path: &Path, current_dir: &Path) {}

    pub fn execute_local(mut self, handler: CopyMode) -> Result<(), io::Error> {
        let handler_function = match handler {
            CopyMode::Copy => |src: &Path, dst: &Path| fs::copy(src, dst).map(|_| ()),
            CopyMode::Symlink => |src: &Path, dst: &Path| os::unix::fs::symlink(src, dst),
        };
        let (workdir, _input_dir, script_path) = self.prep_script_inputs(handler_function)?;
        let log_path = workdir.join(".maestro.log");
        let log_stderr_path = workdir.join(".maestro.err");
        let vars: Vec<_> = self
            .vars
            .iter()
            .map(|(k, val)| match val {
                Injection::Param(s) => (k, s.to_string()),
                Injection::File(p) => (k, p.to_string_lossy().to_string()),
            })
            .collect();
        let mut log_handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        // TODO! Chrono
        writeln!(log_handle, ":: Spawning process script")?;
        writeln!(log_handle, ":: Logging process output...")?;
        let output = Command::new(script_path)
            .stdout(log_handle.try_clone()?)
            .stderr(File::create(&log_stderr_path)?)
            .envs(vars)
            .current_dir(workdir)
            .output()?;
        if !output.status.success() {
            writeln!(
                log_handle,
                ":: Process failed with exit code {:?}",
                output.status.code()
            )?;
            writeln!(
                log_handle,
                ":: Process stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )?;
        } else {
            writeln!(log_handle, ":: Process terminated successfully!")?;
        }
        Ok(())
    }
}
