use std::{
    env,
    fs::{self, File},
    path::Path,
    process::{Command, Stdio},
};

use session_gen::generate_session_id;

use crate::{StringResult, cache::prep_cache, mapper, report_process_failure};

pub(crate) enum BuildType {
    Run(bool),
    Build,
}

pub(crate) fn build_project(additional_args: Vec<String>, ty: BuildType) -> StringResult {
    let cache_dir = prep_cache()?;
    let mut build_cmd = Command::new("cargo");
    build_cmd
        .env(
            "RUSTFLAGS",
            format!(
                "-L {} --extern maestro={}",
                cache_dir.join("deps/").display(),
                cache_dir.join("libmaestro.rlib").display()
            ),
        )
        .arg(match ty {
            BuildType::Run(_) => "run",
            BuildType::Build => "build",
        })
        .args(["--release", "--no-default-features"])
        .args(additional_args);

    if let BuildType::Run(true) = ty {
        let session_id = generate_session_id();
        let workdir = Path::new(&env::var("MAESTRO_WORKDIR").unwrap_or("maestro_work".to_string()))
            .join(&session_id);
        fs::create_dir_all(&workdir)
            .map_err(|e| mapper(&e, "Failed to initialize session workdir"))?;

        let log_file = workdir.join("out.log");
        let err_file = workdir.join("err.log");
        let log_handle =
            File::create(log_file).map_err(|e| mapper(&e, "Failed to create log file"))?;
        let err_handle =
            File::create(err_file).map_err(|e| mapper(&e, "Failed to create err file"))?;
        build_cmd
            .env("MAESTRO_SESSION_ID", session_id)
            .stdout(log_handle)
            .stderr(err_handle)
            .spawn()
            .map_err(|e| mapper(&e, "Failed to spawn background child process"))?;
    } else {
        let cmd = build_cmd
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| mapper(&e, "Failed to build project"))?;

        if !cmd.success() {
            let msg = match ty {
                BuildType::Run(_) => "Running project",
                BuildType::Build => "Building project",
            };
            return Err(report_process_failure(cmd, msg));
        }
    }
    Ok(())
}
