use crate::types::workflow::Workflow;
use std::path::PathBuf;

pub struct Sif(&'static str);
pub struct Oci(&'static str);
pub struct ApptainerScript(&'static str);
pub struct PodmanScript(&'static str);

impl From<Sif> for ApptainerScript {
    fn from(image: Sif) -> Self {
        ApptainerScript(image.0)
    }
}
impl From<Oci> for ApptainerScript {
    fn from(image: Oci) -> Self {
        ApptainerScript(image.0)
    }
}
impl From<Oci> for PodmanScript {
    fn from(image: Oci) -> Self {
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
