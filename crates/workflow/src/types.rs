use std::path::PathBuf;

pub struct Workflow {
    pub cmd: Script,
    pub outputs: Vec<PathBuf>,
}

pub struct Script {
    pub contents: &'static str,
    pub env: Vec<EnvVar>,
    pub runtime: Runtime,
}

pub struct EnvVar(&'static str, EnvVarValue);
pub enum EnvVarValue {
    Param(String),
    File(PathBuf),
}
impl From<String> for EnvVarValue {
    fn from(s: String) -> Self {
        Self::Param(s)
    }
}
impl From<PathBuf> for EnvVarValue {
    fn from(p: PathBuf) -> Self {
        Self::File(p)
    }
}

pub enum Runtime {
    Local,
    Container(Container),
}
pub enum Container {
    Apptainer(&'static str),
}
