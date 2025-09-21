use crate::{executors::GenericExecutor, session::setup_session_workdir};
use ctor::ctor;
pub use inventory::submit as submit_request;
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    fs::{self},
    path::PathBuf,
    process::exit,
    sync::LazyLock,
};

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[doc(hidden)]
pub struct TomlConfig {
    pub executor: GenericExecutor,
    #[serde(default)]
    pub custom_executor: HashMap<String, MaybeAliasedExecutor>,
    pub args: HashMap<String, String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum MaybeAliasedExecutor {
    Alias { alias: String },
    Executor(GenericExecutor),
}

pub struct MaestroConfig {
    pub executor: GenericExecutor,
    pub custom_executors: HashMap<String, GenericExecutor>,
    pub args: HashMap<String, String>,
}

pub static MAESTRO_CONFIG: LazyLock<MaestroConfig> = LazyLock::new(|| {
    let config_file = env::var("MAESTRO_CONFIG").unwrap_or("Maestro.toml".to_string());
    let file_contents = fs::read_to_string(config_file).unwrap_or_else(|e| {
        eprintln!("Failed to read config file: {e}");
        exit(1)
    });
    let mut config: TomlConfig = toml::from_str(&file_contents).unwrap_or_else(|e| {
        eprintln!("Failed to parse Maestro.toml: {e}");
        exit(1)
    });
    config.custom_executor.insert(
        "default".to_string(),
        MaybeAliasedExecutor::Executor(config.executor.clone()),
    );

    let canonicalized_executors = config
        .custom_executor
        .iter()
        .map(|(name, exec)| {
            let executor = match exec {
                MaybeAliasedExecutor::Executor(exec) => exec.clone(),
                MaybeAliasedExecutor::Alias { alias } => {
                    let canonical_path = config.custom_executor.get(alias).unwrap_or_else(|| {
                        eprintln!(r#"Unable to resolve executor alias "{alias}""#);
                        exit(1)
                    });
                    match canonical_path {
                        MaybeAliasedExecutor::Executor(exe) => exe.clone(),
                        MaybeAliasedExecutor::Alias { .. } => {
                            eprintln!(
                                r#"Chained aliases ("{name}" -> "{alias}" -> ..) are not accepted"#
                            );
                            exit(1)
                        }
                    }
                }
            };
            (name.clone(), executor)
        })
        .collect();

    MaestroConfig {
        executor: config.executor,
        custom_executors: canonicalized_executors,
        args: config.args,
    }
});

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
        $crate::MAESTRO_CONFIG.custom_executors[$name].exe($process)
    }};
}

#[macro_export]
macro_rules! arg {
    ($arg:literal) => {{
        $crate::submit_request! {
            $crate::RequestedArg($arg, file!(), line!(), column!())
        };
        &$crate::MAESTRO_CONFIG.args[$arg]
    }};
}

#[ctor]
fn initialize() {
    LazyLock::force(&MAESTRO_CONFIG);
    for RequestedExecutor(name, file, line, col) in inventory::iter::<RequestedExecutor> {
        if !MAESTRO_CONFIG.custom_executors.contains_key(*name) {
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
