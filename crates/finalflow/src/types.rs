mod macros;
use std::path::PathBuf;

pub struct Workflow {
    pub cmd: Script,
    pub outputs: Vec<PathBuf>,
}

pub struct Script {
    pub contents: &'static str,
    pub env: Vec<EnvVar>,
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

pub struct SIF(&'static str);
pub struct OCI(&'static str);
pub struct ApptainerScript(&'static str);
pub struct PodmanScript(&'static str);

impl From<SIF> for ApptainerScript {
    fn from(image: SIF) -> Self {
        ApptainerScript(image.0)
    }
}
impl From<OCI> for ApptainerScript {
    fn from(image: OCI) -> Self {
        ApptainerScript(image.0)
    }
}
impl From<OCI> for PodmanScript {
    fn from(image: OCI) -> Self {
        PodmanScript(image.0)
    }
}

pub trait ContainerWorkflow {
    fn execute(self) -> Vec<PathBuf>;
}
pub struct ApptainerWorkflow(Workflow, ApptainerScript);
pub struct PodmanWorkflow(Workflow, PodmanScript);

// todo!: implement
// ```rust
//      using_apptainer(&self, impl Into<ApptainerScript>) -> ApptainerWorkflow
//      using_podman(&self, impl Into<PodmanScript>) -> PodmanWorkflow
// ```
// for Workflow
