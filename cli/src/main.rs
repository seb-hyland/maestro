use crate::{
    build::{BuildType, build_project},
    bundle::build_and_bundle,
    cache::prep_cache,
    init::initialize,
};
use clap::{Parser, Subcommand};
use std::{
    borrow::Cow,
    env,
    error::Error,
    path::PathBuf,
    process::{self, Command, ExitStatus},
};

mod build;
mod bundle;
mod cache;
mod init;

fn main() {
    let command = Cmd::parse();
    if let Err(e) = match command {
        Cmd::Init { path } => initialize(path),
        Cmd::Bundle { args } => build_and_bundle(args),
        Cmd::UpgradeCache => prep_cache().map(|_| {}),
        Cmd::Build { args } => build_project(args, BuildType::Build),
        Cmd::Run { args } => build_project(args, BuildType::Run),
    } {
        eprintln!("{e}");
        process::exit(1);
    }
}

#[derive(Parser)]
#[command(version, about)]
enum Cmd {
    Init {
        path: Option<String>,
    },
    Bundle {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    UpgradeCache,
    Build {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Run {
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

fn dedent<S: ToString>(s: S) -> Vec<u8> {
    let str = s.to_string();
    str.trim()
        .lines()
        .map(|line| line.trim_start().as_bytes().to_owned())
        .reduce(|mut acc, l| {
            acc.push(b'\n');
            acc.extend_from_slice(&l);
            acc
        })
        .unwrap()
}

fn rustc_version() -> Result<String, StringErr> {
    let rustc_version_cmd = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|e| mapper(&e, "Failed to determine rustc version"))?;
    if !rustc_version_cmd.status.success() {
        return Err(report_process_failure(
            rustc_version_cmd.status,
            "Determining rustc version",
        ));
    }
    let rustc_output = String::from_utf8_lossy(&rustc_version_cmd.stdout);
    rustc_output
        .lines()
        .last()
        .ok_or(static_err("Failed to parse empty rustc output"))?
        .split_whitespace()
        .nth(1)
        .ok_or(static_err("Failed to parse rustc version output"))
        .map(|v| v.to_string())
}
