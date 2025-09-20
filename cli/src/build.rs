use std::process::{Command, Stdio};

use crate::{StringResult, cache::prep_cache, mapper, report_process_failure};

pub(crate) fn build_project(additional_args: Vec<String>) -> StringResult {
    let cache_dir = prep_cache()?;
    let build_cmd = Command::new("cargo")
        .env(
            "RUSTFLAGS",
            format!(
                "-L {} --extern maestro={}",
                cache_dir.join("deps/").display(),
                cache_dir.join("libmaestro.rlib").display()
            ),
        )
        .args(["build", "--release", "--no-default-features"])
        .args(additional_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| mapper(&e, "Failed to build project"))?;

    if !build_cmd.success() {
        return Err(report_process_failure(build_cmd, "Building project"));
    }
    Ok(())
}
