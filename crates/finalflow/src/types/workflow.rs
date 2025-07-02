use std::path::PathBuf;

pub struct Workflow {
    pub cmd: Script,
    pub outputs: Vec<PathBuf>,
}

pub struct Script {
    pub contents: &'static str,
    pub env: Vec<EnvVar>,
}

pub struct EnvVar(pub &'static str, pub EnvVarValue);

pub enum EnvVarValue {
    Param(String),
    File(PathBuf),
}
impl From<String> for EnvVarValue {
    fn from(s: String) -> Self {
        Self::Param(s)
    }
}
impl From<&str> for EnvVarValue {
    fn from(s: &str) -> Self {
        Self::Param(s.to_string())
    }
}
impl From<PathBuf> for EnvVarValue {
    fn from(p: PathBuf) -> Self {
        Self::File(p)
    }
}
impl From<&PathBuf> for EnvVarValue {
    fn from(p: &PathBuf) -> Self {
        Self::File(p.clone())
    }
}

pub type ExecutionResult = Result<Vec<PathBuf>, ExecutionError>;

#[derive(Debug)]
pub enum ExecutionError {
    DirectoryError(String),
    WriteError(String),
    ProcessError(String),
    InputsNotFound(Vec<PathBuf>),
    OutputsNotFound(Vec<PathBuf>),
}
