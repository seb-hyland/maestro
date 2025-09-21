use crate::{StringResult, mapper, report_process_failure, static_err};
use std::{fs, path::Path, process::Command};

pub(crate) fn kill_process(path: &Path) -> StringResult {
    let mut marker_file_path = Path::new(&path).join(".maestro.active");
    if !marker_file_path.exists() {
        marker_file_path = Path::new("maestro_work").join(path).join(".maestro.active");
    }
    if !marker_file_path.exists() {
        return Err(static_err(
            "Path does not exist. The process may have completed or the path may be malformed.",
        ));
    }

    let pid: u64 = fs::read_to_string(&marker_file_path)
        .map_err(|e| mapper(&e, "Failed to read .maestro.active"))?
        .parse()
        .map_err(|e| mapper(&e, "Failed to parse PID from .maestro.active"))?;
    let cmd = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .status()
        .map_err(|e| mapper(&e, "Failed to kill maestro process"))?;
    if !cmd.success() {
        return Err(report_process_failure(cmd, "Killing maestro process"));
    }

    fs::remove_file(&marker_file_path)
        .map_err(|e| mapper(&e, "Failed to remove .maestro.active"))?;
    Ok(())
}
