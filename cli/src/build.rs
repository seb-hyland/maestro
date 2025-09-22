use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use session_gen::generate_session_id;

use crate::{StringErr, StringResult, cache::prep_cache, dedent, mapper, report_process_failure};

pub(crate) enum BuildType {
    Run {
        background: bool,
        binary: Option<PathBuf>,
    },
    Build,
}

pub(crate) fn build_project(
    cargo_args: Vec<String>,
    program_args: Vec<String>,
    ty: BuildType,
) -> StringResult {
    fn setup_cargo_env(
        cmd: &mut Command,
        run: bool,
        cache_dir: &Path,
        cargo_args: Vec<String>,
        program_args: Vec<String>,
    ) {
        cmd.env(
            "RUSTFLAGS",
            format!(
                "-L {} --extern maestro={}",
                cache_dir.join("deps/").display(),
                cache_dir.join("libmaestro.rlib").display()
            ),
        )
        .arg(if run { "run" } else { "build" })
        .args(["--release", "--no-default-features"])
        .args(cargo_args)
        .arg("--")
        .args(program_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    }

    fn setup_maestro_workdir(cmd: &mut Command) -> Result<String, StringErr> {
        let session_id = generate_session_id();
        let workdir = Path::new(&env::var("MAESTRO_WORKDIR").unwrap_or("maestro_work".to_string()))
            .join(&session_id);
        let log_path = workdir.join("out.log");
        let err_path = workdir.join("err.log");

        fs::create_dir_all(&workdir)
            .map_err(|e| mapper(&e, "Failed to initialize session workdir"))?;
        let log_handle =
            File::create(&log_path).map_err(|e| mapper(&e, "Failed to create log file"))?;
        let err_handle =
            File::create(&err_path).map_err(|e| mapper(&e, "Failed to create err file"))?;

        cmd.env("MAESTRO_SESSION_ID", &session_id)
            .stdout(log_handle)
            .stderr(err_handle);

        Ok(dedent(format!(
            r#"
            maestro process spawned in background
            Session ID: {}
            Workdir: {}
            stdout: {}
            stderr: {}
        "#,
            session_id,
            workdir.display(),
            log_path.display(),
            err_path.display()
        )))
    }

    match ty {
        BuildType::Build => {
            let cache_dir = prep_cache()?;

            let mut build_cmd = Command::new("cargo");
            setup_cargo_env(&mut build_cmd, false, &cache_dir, cargo_args, program_args);
            let status = build_cmd
                .status()
                .map_err(|e| mapper(&e, "Failed to build project"))?;
            if !status.success() {
                return Err(report_process_failure(status, "Building project"));
            }
        }
        BuildType::Run { background, binary } => match binary {
            None => {
                let cache_dir = prep_cache()?;

                let mut run_cmd = Command::new("cargo");
                setup_cargo_env(&mut run_cmd, true, &cache_dir, cargo_args, program_args);
                if !background {
                    let status = run_cmd
                        .status()
                        .map_err(|e| mapper(&e, "Failed to run project"))?;
                    if !status.success() {
                        return Err(report_process_failure(status, "Running project"));
                    }
                } else {
                    let print_stmt = setup_maestro_workdir(&mut run_cmd)?;
                    run_cmd
                        .spawn()
                        .map_err(|e| mapper(&e, "Failed to spawn background child process"))?;
                    println!("{}", print_stmt);
                }
            }
            Some(bin) => {
                let bin_path = bin
                    .canonicalize()
                    .map_err(|e| mapper(&e, "Failed to canonicalize binary path"))?;
                let mut run_cmd = Command::new(bin_path);
                if !background {
                    let status = run_cmd
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .status()
                        .map_err(|e| mapper(&e, "Failed to run binary"))?;
                    if !status.success() {
                        return Err(report_process_failure(status, "Running binary"));
                    }
                } else {
                    let print_stmt = setup_maestro_workdir(&mut run_cmd)?;
                    run_cmd
                        .spawn()
                        .map_err(|e| mapper(&e, "Failed to spawn background child process"))?;
                    println!("{}", print_stmt);
                }
            }
        },
    };

    Ok(())
}
