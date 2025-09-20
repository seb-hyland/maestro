use crate::{build::build_project, bundle::build_and_bundle, cache::prep_cache, init::initialize};
use clap::Parser;
use std::{
    borrow::Cow,
    env,
    error::Error,
    path::PathBuf,
    process::{self, ExitStatus},
};

mod build;
mod bundle;
mod cache;
mod init;

fn main() {
    let command = SubCommand::parse();
    if let Err(e) = match command {
        SubCommand::Init { path } => initialize(path),
        SubCommand::Bundle { args } => build_and_bundle(args),
        SubCommand::InstallCache => prep_cache().map(|_| {}),
        SubCommand::Build { args } => build_project(args),
    } {
        eprintln!("{e}");
        process::exit(1);
    }
}

#[derive(Parser)]
#[command(version, about)]
enum SubCommand {
    Init {
        path: Option<String>,
    },
    Bundle {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    InstallCache,
    Build {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

const TOOLCHAIN_VERSION: &str = "1.90.0";

type StringErr = Cow<'static, str>;
type StringResult = Result<(), StringErr>;

fn mapper(e: &dyn Error, msg: &'static str) -> StringErr {
    Cow::Owned(format!("{msg}: {e}"))
}
fn static_err(msg: &'static str) -> StringErr {
    Cow::Borrowed(msg)
}
fn dynamic_err(msg: String) -> StringErr {
    Cow::Owned(msg)
}

fn find_crate_root() -> Result<PathBuf, StringErr> {
    let working_directory =
        env::current_dir().map_err(|e| mapper(&e, "Failed to identify working directory"))?;
    let mut current_dir = working_directory;
    while !current_dir.join("Cargo.toml").exists() {
        current_dir = match current_dir.parent() {
            Some(parent) => parent.to_path_buf(),
            None => {
                return Err(static_err(
                    "Failed to find Cargo.toml in a parent of the working directory",
                ));
            }
        }
    }
    Ok(current_dir)
}

fn report_process_failure(status: ExitStatus, process: &'static str) -> StringErr {
    Cow::Owned(match status.code() {
        Some(code) => format!("{process} failed with exit code {code}"),
        None => format!("{process} failed due to external signal"),
    })
}
