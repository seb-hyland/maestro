use std::{fs, path::Path, process::Command};

use crate::{StringResult, dynamic_err, mapper, report_process_failure};

pub(crate) fn kill_process(path: &Path) -> StringResult {
    let path = Path::new(&path).join(".maestro.active");
    if !path.exists() {
        return Err(dynamic_err(format!(
            "Path {} does not exist. The process may have completed or the path may be malformed.",
            path.display()
        )));
    }

    let pid: u64 = fs::read_to_string(&path)
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

    fs::remove_file(&path).map_err(|e| mapper(&e, "Failed to remove .maestro.active"))?;
    Ok(())
}
