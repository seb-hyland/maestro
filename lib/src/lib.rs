use crate::executors::{Executor, generic::GenericExecutor};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    fs::{self},
    io::{self},
    ops::Index,
    path::PathBuf,
    sync::LazyLock,
};

pub mod executors;
mod macros;
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
impl Container {
    pub fn from_docker(image: &'static str) -> Self {
        Self::Docker(Cow::Borrowed(image))
    }
    pub fn from_apptainer(image: &'static str) -> Self {
        Self::Apptainer(Cow::Borrowed(image))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaestroConfig {
    executor: GenericExecutor,
    args: HashMap<String, String>,
}
impl MaestroConfig {
    pub fn execute(&self, process: Process) -> io::Result<Vec<PathBuf>> {
        match &self.executor {
            GenericExecutor::Local(executor) => executor.exe(process),
            GenericExecutor::Slurm(executor) => executor.exe(process),
        }
    }
    pub fn get(&self, arg: &str) -> Option<&str> {
        self.args.get(arg).map(|v| v.as_str())
    }
}
impl Index<&str> for MaestroConfig {
    type Output = String;
    fn index(&self, index: &str) -> &Self::Output {
        &self.args[index]
    }
}
pub static MAESTRO_CONFIG: LazyLock<MaestroConfig> = LazyLock::new(|| {
    let config_file = env::var("MAESTRO_CONFIG").unwrap_or("Maestro.toml".to_string());
    let file_contents = fs::read_to_string(config_file).expect("Failed to read config file!");
    toml::from_str(&file_contents).unwrap_or_else(|e| panic!("Failed to parse Maestro.toml: {e}"))
});
