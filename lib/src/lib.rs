use crate::{config::MAESTRO_CONFIG, session::setup_session_workdir};
use ctor::ctor;
pub use inventory::submit as submit_request;
use std::{borrow::Cow, path::PathBuf, process::exit, sync::LazyLock};

pub mod config;
pub mod executors;
pub mod prelude;
pub mod process;
mod session;

const LP: &str = "\x1b[0;34m::\x1b[0m";

pub type PathArg = (Cow<'static, str>, PathBuf);
pub type StrArg = (Cow<'static, str>, String);

#[derive(Clone)]
pub struct Process {
    name: String,
    container: Option<Container>,
    inputs: Vec<PathArg>,
    args: Vec<StrArg>,
    outputs: Vec<PathArg>,
    script: Cow<'static, str>,
}

#[derive(Clone)]
pub enum Container {
    Docker(Cow<'static, str>),
    Apptainer(Cow<'static, str>),
}

pub struct RequestedExecutor(pub &'static str, pub &'static str, pub u32, pub u32);
inventory::collect!(RequestedExecutor);
pub struct RequestedArg(pub &'static str, pub &'static str, pub u32, pub u32);
inventory::collect!(RequestedArg);

#[macro_export]
macro_rules! execute {
    ($process:expr) => {{ $crate::MAESTRO_CONFIG.executor.exe($process) }};
    ($name:literal, $process:expr) => {{
        $crate::submit_request! {
            $crate::RequestedExecutor($name, file!(), line!(), column!())
        };
        $crate::config::MAESTRO_CONFIG.executors[$name].exe($process)
    }};
}

#[macro_export]
macro_rules! arg {
    ($arg:literal) => {{
        $crate::submit_request! {
            $crate::RequestedArg($arg, file!(), line!(), column!())
        };
        &$crate::config::MAESTRO_CONFIG.args[$arg]
    }};
}

#[ctor]
fn initialize() {
    LazyLock::force(&MAESTRO_CONFIG);
    for RequestedExecutor(name, file, line, col) in inventory::iter::<RequestedExecutor> {
        if !MAESTRO_CONFIG.executors.contains_key(*name) {
            eprintln!(
                "Custom executor \"{name}\" expected to be defined in Maestro.toml.\nLocation: {file}:{line}:{col}"
            );
            exit(1)
        }
    }
    for RequestedArg(arg, file, line, col) in inventory::iter::<RequestedArg> {
        if !MAESTRO_CONFIG.args.contains_key(*arg) {
            eprintln!(
                "Arg \"{arg}\" expected to be defined in Maestro.toml.\nLocation: {file}:{line}:{col}"
            );
            exit(1)
        }
    }
    let workdir = match setup_session_workdir() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to setup session workdir: {e}");
            exit(1)
        }
    };
    let _ = session::SESSION_WORKDIR.set(workdir);
}
