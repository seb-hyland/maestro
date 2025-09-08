use crate::{Injection, Script, session::create_process_dir};
use std::{
    fs::{self, File, OpenOptions, create_dir},
    io::{self, Write},
    os::unix::{self, fs::OpenOptionsExt},
    path::{Path, PathBuf},
};
#[derive(Clone, Copy)]
pub enum CopyMode {
    Copy,
    Symlink,
}

impl<'a> Script<'a> {
    pub(crate) fn prep_script_inputs(
        &mut self,
        copy_mode: CopyMode,
    ) -> Result<(PathBuf, PathBuf, File), io::Error> {
        let process_workdir = create_process_dir()?;
        let mut log_handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(process_workdir.join(".maestro.log"))?;
        writeln!(
            log_handle,
            ":: Process workdir initialized at {}",
            process_workdir.display()
        )?;
        let input_dir = process_workdir.join(".maestro_inputs");
        create_dir(&input_dir)?;

        writeln!(
            log_handle,
            ":: Staging inputs using method {}",
            match copy_mode {
                CopyMode::Copy => "COPY",
                CopyMode::Symlink => "SYMLINK",
            }
        )?;
        for (var, injection) in self.vars.iter_mut() {
            if let Injection::File(origin_path) = injection
                && origin_path.exists()
            {
                let name = origin_path.file_name().ok_or(io::Error::new(
                    io::ErrorKind::InvalidFilename,
                    format!("No file name for path {origin_path:?}"),
                ))?;
                let destination = input_dir.join(format!("[{}]{}", var, name.display()));
                Self::injection_transformer(origin_path, &destination, copy_mode)?;
                writeln!(
                    log_handle,
                    "Input for variable {var} at {} copied to {}",
                    origin_path.display(),
                    destination.display()
                )?;
                *injection = Injection::File(destination);
            }
        }

        let script_path = process_workdir.join(".maestro.sh");
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(&script_path)?;
        script_file.write_all(self.script.as_bytes())?;

        Ok((process_workdir, script_path, log_handle))
    }

    fn injection_transformer(
        item_path: &Path,
        target_path: &Path,
        copy_mode: CopyMode,
    ) -> io::Result<()> {
        // If passed a file
        if !item_path.is_dir() {
            match copy_mode {
                CopyMode::Copy => fs::copy(item_path, target_path).map(|_| ())?,
                CopyMode::Symlink => unix::fs::symlink(item_path, target_path)?,
            }
            return Ok(());
        }

        // If passed a directory
        create_dir(target_path)?;
        let dir_contents = item_path.read_dir()?;
        for item in dir_contents.flatten() {
            let entry_path = item.path();
            let entry_name = entry_path.file_name().ok_or(io::Error::new(
                io::ErrorKind::InvalidFilename,
                format!("No filename for path {entry_path:?}"),
            ))?;
            let target = target_path.join(entry_name);
            Self::injection_transformer(&entry_path, &target, copy_mode)?;
        }

        Ok(())
    }
}
