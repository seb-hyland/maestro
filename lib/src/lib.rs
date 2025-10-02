use crate::{
    config::MAESTRO_CONFIG,
    session::{SESSION_WORKDIR, setup_session_workdir},
};
use dagger_lib::result::NodeResult;
pub use inventory::submit as submit_request;
pub use maestro_macros::main;
use serde::Deserialize;
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    process::exit,
    sync::LazyLock,
};

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
    inputs: Vec<PathArg>,
    args: Vec<StrArg>,
    outputs: Vec<PathArg>,
    script: Cow<'static, str>,
}

#[derive(Clone, Deserialize)]
pub enum Container {
    Docker(Cow<'static, str>),
    Apptainer(Cow<'static, str>),
    Podman(Cow<'static, str>),
}

pub type WorkflowResult = NodeResult<Vec<PathBuf>>;

pub trait IntoArray<T, const N: usize> {
    fn into_array(self) -> [T; N];
}
impl<T, const N: usize> IntoArray<T, N> for Vec<T> {
    fn into_array(mut self) -> [T; N] {
        let len = self.len();
        assert!(
            len >= N,
            "Vector does not have enough elements to coerce into array of length {N}"
        );
        self.truncate(N);
        self.try_into()
            .unwrap_or_else(|_| unreachable!("Vector will be exactly N elements"))
    }
}

pub struct RequestedExecutor(pub &'static str, pub &'static str, pub u32, pub u32);
inventory::collect!(RequestedExecutor);

pub struct RequestedArg(pub &'static str, pub &'static str, pub u32, pub u32);
inventory::collect!(RequestedArg);

pub struct RequestedInputFiles(pub &'static str, pub &'static str, pub u32, pub u32);
inventory::collect!(RequestedInputFiles);

#[macro_export]
macro_rules! arg {
    ($arg:literal) => {{
        $crate::submit_request! {
            $crate::RequestedArg($arg, file!(), line!(), column!())
        };
        &$crate::config::MAESTRO_CONFIG.args[$arg]
    }};
}

#[macro_export]
macro_rules! inputs {
    ($input:literal) => {{
        $crate::submit_request! {
            $crate::RequestedInputFiles($input, file!(), line!(), column!())
        };
        &$crate::config::MAESTRO_CONFIG.inputs[$input]
            .iter()
            .map(::std::path::Path::new)
            .collect::<Box<[&Path]>>()[..]
    }};
}

pub fn initialize() {
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
    for RequestedInputFiles(arg, file, line, col) in inventory::iter::<RequestedInputFiles> {
        if !MAESTRO_CONFIG.inputs.contains_key(*arg) {
            eprintln!(
                "Input argument \"{arg}\" expected to be defined in Maestro.toml.\nLocation: {file}:{line}:{col}"
            );
            exit(1)
        }
        let files = &MAESTRO_CONFIG.inputs[*arg];
        for file in files {
            if !Path::new(file).exists() {
                eprintln!("Input file \"{}\" does not exist!", file);
                exit(1)
            }
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

pub fn deinitialize() {
    if let Some(dir) = SESSION_WORKDIR.get() {
        let _ = fs::remove_file(dir.join(".maestro.active"));
    }
}

// #[macro_export]
// macro_rules! execute {
//     ($process:expr) => {{ $crate::MAESTRO_CONFIG.executor.exe($process) }};
//     ($name:literal, $process:expr) => {{
//         $crate::submit_request! {
//             $crate::RequestedExecutor($name, file!(), line!(), column!())
//         };
//         $crate::config::MAESTRO_CONFIG.executors[$name].exe($process)
//     }};
// }
